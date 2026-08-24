#![forbid(unsafe_code)]

use std::collections::HashMap;

use blob_core::{CausalRecord, CausalRecordId};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HistoryError {
    DuplicateId(CausalRecordId),
    UnknownParent(CausalRecordId),
}

#[derive(Default)]
pub struct InMemoryCausalLog {
    records: Vec<CausalRecord>,
    index: HashMap<CausalRecordId, usize>,
}

impl InMemoryCausalLog {
    pub fn append(&mut self, record: CausalRecord) -> Result<(), HistoryError> {
        if self.index.contains_key(&record.id) {
            return Err(HistoryError::DuplicateId(record.id));
        }

        for parent in &record.parents {
            if !self.index.contains_key(parent) {
                return Err(HistoryError::UnknownParent(parent.clone()));
            }
        }

        let position = self.records.len();
        self.index.insert(record.id.clone(), position);
        self.records.push(record);
        Ok(())
    }

    pub fn get(&self, id: &CausalRecordId) -> Option<&CausalRecord> {
        self.index.get(id).and_then(|position| self.records.get(*position))
    }

    pub fn records(&self) -> &[CausalRecord] {
        &self.records
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}
