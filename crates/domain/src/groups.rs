//! Gruppi di avvio (v0.6.0): un nome e un elenco ordinato di app, aperte in un gesto.
//!
//! Il gruppo non avvia nulla da sé: l'avvio passa, app per app, dall'unica funzione di avvio
//! del guscio, con le sue verifiche (A.7.3).

use std::collections::HashSet;

use rusqlite::params;
use uuid::Uuid;

use crate::registry::{parse_uuid, validate_name};
use crate::{Database, Error};

/// App al massimo in un gruppo: si aprono tutte insieme, e il limite evita di aprirne decine
/// per errore.
pub const GROUP_MAX_APPS: usize = 20;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    pub id: Uuid,
    pub name: String,
    /// Nell'ordine di avvio.
    pub app_ids: Vec<Uuid>,
}

#[derive(Debug, Clone)]
pub struct GroupInput {
    pub name: String,
    pub app_ids: Vec<Uuid>,
}

/// Nome con le stesse regole delle app; niente doppioni, al massimo `GROUP_MAX_APPS` app.
/// Che le app esistano lo verifica il database (chiave esterna, `NOT_FOUND`).
pub fn validate_group(input: &GroupInput) -> Result<String, Error> {
    let name = validate_name(&input.name)?;
    if input.app_ids.len() > GROUP_MAX_APPS {
        return Err(Error::GroupTooLarge);
    }
    let mut seen = HashSet::new();
    if !input.app_ids.iter().all(|id| seen.insert(*id)) {
        return Err(Error::GroupAppDuplicate);
    }
    Ok(name)
}

impl Database {
    /// Gruppi per nome, ciascuno con le sue app in ordine di avvio.
    pub fn groups(&self) -> Result<Vec<Group>, Error> {
        let mut groups = self
            .conn
            .prepare("SELECT id, name FROM launch_groups ORDER BY name COLLATE NOCASE")?
            .query_map([], |row| {
                Ok(Group {
                    id: parse_uuid(row, 0)?,
                    name: row.get(1)?,
                    app_ids: Vec::new(),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut statement = self
            .conn
            .prepare("SELECT group_id, app_id FROM launch_group_apps ORDER BY position")?;
        for member in
            statement.query_map([], |row| Ok((parse_uuid(row, 0)?, parse_uuid(row, 1)?)))?
        {
            let (group_id, app_id) = member?;
            if let Some(group) = groups.iter_mut().find(|group| group.id == group_id) {
                group.app_ids.push(app_id);
            }
        }
        Ok(groups)
    }

    pub fn group(&self, id: Uuid) -> Result<Group, Error> {
        self.groups()?
            .into_iter()
            .find(|group| group.id == id)
            .ok_or(Error::NotFound)
    }

    pub fn create_group(&self, input: &GroupInput) -> Result<Group, Error> {
        let id = Uuid::now_v7();
        self.write_group(id, input, true)?;
        self.group(id)
    }

    pub fn update_group(&self, id: Uuid, input: &GroupInput) -> Result<Group, Error> {
        self.write_group(id, input, false)?;
        self.group(id)
    }

    /// Nome ed elenco in una sola transazione: o tutto, o niente (A.7.3).
    fn write_group(&self, id: Uuid, input: &GroupInput, is_new: bool) -> Result<(), Error> {
        let name = validate_group(input)?;
        let tx = self.conn.unchecked_transaction()?;
        let changed = if is_new {
            tx.execute(
                "INSERT INTO launch_groups (id, name) VALUES (?1, ?2)",
                params![id.to_string(), name],
            )?
        } else {
            tx.execute(
                "UPDATE launch_groups SET name = ?2 WHERE id = ?1",
                params![id.to_string(), name],
            )?
        };
        if changed == 0 {
            return Err(Error::NotFound);
        }
        tx.execute(
            "DELETE FROM launch_group_apps WHERE group_id = ?1",
            params![id.to_string()],
        )?;
        for (position, app_id) in input.app_ids.iter().enumerate() {
            let position = i64::try_from(position).map_err(|_| Error::GroupTooLarge)?;
            tx.execute(
                "INSERT INTO launch_group_apps (group_id, app_id, position) VALUES (?1, ?2, ?3)",
                params![id.to_string(), app_id.to_string(), position],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn delete_group(&self, id: Uuid) -> Result<(), Error> {
        let deleted = self.conn.execute(
            "DELETE FROM launch_groups WHERE id = ?1",
            params![id.to_string()],
        )?;
        if deleted == 0 {
            return Err(Error::NotFound);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::tests::{open_db, web};

    fn input(name: &str, app_ids: &[Uuid]) -> GroupInput {
        GroupInput {
            name: name.into(),
            app_ids: app_ids.to_vec(),
        }
    }

    #[test]
    fn a_group_keeps_its_apps_in_launch_order() {
        let (_dir, db) = open_db();
        let a = db.create_app(&web("Alfa")).unwrap().id;
        let b = db.create_app(&web("Beta")).unwrap().id;
        let group = db.create_group(&input(" Mattino ", &[b, a])).unwrap();
        assert_eq!(group.name, "Mattino");
        assert_eq!(group.app_ids, [b, a]);
        let group = db.update_group(group.id, &input("Mattino", &[a])).unwrap();
        assert_eq!(group.app_ids, [a]);
        assert_eq!(db.groups().unwrap(), std::slice::from_ref(&group));
        db.delete_group(group.id).unwrap();
        assert!(db.groups().unwrap().is_empty());
        assert_eq!(db.registry().unwrap().apps.len(), 2, "le app restano");
        assert!(matches!(db.delete_group(group.id), Err(Error::NotFound)));
    }

    #[test]
    fn duplicates_too_many_apps_and_unknown_apps_are_refused_without_writing() {
        let (_dir, db) = open_db();
        let a = db.create_app(&web("Alfa")).unwrap().id;
        assert!(matches!(
            db.create_group(&input("G", &[a, a])),
            Err(Error::GroupAppDuplicate)
        ));
        let many: Vec<Uuid> = (0..=GROUP_MAX_APPS).map(|_| Uuid::now_v7()).collect();
        assert!(matches!(
            db.create_group(&input("G", &many)),
            Err(Error::GroupTooLarge)
        ));
        assert!(matches!(
            db.create_group(&input("G", &[a, Uuid::now_v7()])),
            Err(Error::NotFound)
        ));
        assert!(matches!(
            db.create_group(&input("  ", &[a])),
            Err(Error::NameInvalid)
        ));
        assert!(
            db.groups().unwrap().is_empty(),
            "nessuna scrittura parziale"
        );
    }

    #[test]
    fn names_are_unique_regardless_of_case() {
        let (_dir, db) = open_db();
        db.create_group(&input("Mattino", &[])).unwrap();
        assert!(matches!(
            db.create_group(&input("MATTINO", &[])),
            Err(Error::NameDuplicate)
        ));
    }

    #[test]
    fn deleting_an_app_removes_it_from_its_groups() {
        let (_dir, db) = open_db();
        let a = db.create_app(&web("Alfa")).unwrap().id;
        let b = db.create_app(&web("Beta")).unwrap().id;
        let group = db.create_group(&input("G", &[a, b])).unwrap();
        db.delete_app(a).unwrap();
        assert_eq!(db.group(group.id).unwrap().app_ids, [b]);
    }
}
