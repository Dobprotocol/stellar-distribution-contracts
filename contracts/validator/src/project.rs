use soroban_sdk::{Env, BytesN, Vec};

use crate::storage_types::DataKey;
use crate::storage_types::ProjectData;
use crate::storage_types::ProjectStatusEnum;
use crate::storage_types::TrufaScoreValues;

pub fn set_project_status(
    e: &Env,
    project_hash: &BytesN<32>,
    status: &ProjectStatusEnum,
) {
    let project_key = DataKey::ProjectStatus(project_hash.clone());
    // if project already exists, we just update its value
    if e.storage().instance().get(&project_key).unwrap_or(ProjectStatusEnum::NotSet) != ProjectStatusEnum::NotSet {
        e.storage().instance().set(&project_key, status);
        return;
    }
    // else, we set its value andcreate a new index
    e.storage().instance().set(&project_key, status);

    let len = e.storage().instance().get(&DataKey::ProjectIndexLength).unwrap_or(0u32);
    let index_key = DataKey::ProjectIndex(len);
    e.storage().instance().set(&index_key, project_hash);
    e.storage().instance().set(&DataKey::ProjectIndexLength, &(len + 1));
}

pub fn get_project_status(
    e: &Env,
    project_hash: &BytesN<32>,
) -> ProjectStatusEnum {
    let key = DataKey::ProjectStatus(project_hash.clone());
    e.storage().instance().get(&key).unwrap_or(ProjectStatusEnum::NotSet)
}

pub fn get_projects_statuses_in_bulk(
    e: &Env,
    start: u32,
    end: u32
) -> Vec<ProjectData> {
    // this function will scalate and be very expensive in the future

    let mut storage_end = e.storage().instance().get(&DataKey::ProjectIndexLength).unwrap_or(0);
    // if start is more than storage_end or end is lower than start, return an empty vector
    if start > storage_end || end <= start {
        return Vec::new(e);
    }
    if storage_end > end {
        storage_end = end;
    }
    let zero_hash = BytesN::<32>::from_array(&e, &[0u8; 32]);
    let mut result = Vec::new(e);
    for i in start..storage_end {
        // get the hash from the index storage
        let index_key = DataKey::ProjectIndex(i);
        let project_hash = e.storage().instance().get::<_, BytesN<32>>(&index_key).unwrap_or(zero_hash.clone());
        // using the project hash, get the project status
        let project_key = DataKey::ProjectStatus(project_hash.clone());
        let project_status = e.storage().instance().get(&project_key).unwrap_or(ProjectStatusEnum::NotSet);
        // add the project status and hash to the result
        result.push_back(ProjectData {
            hash: project_hash.clone(),
            status: project_status.clone(),
        });
    }
    result
}

pub fn get_number_of_projects(
    e: &Env,
) -> u32 {
    let len = e.storage().instance().get(&DataKey::ProjectIndexLength).unwrap_or(0);
    len
}

pub fn get_projects_statuses_from_vec(
    e: &Env,
    project_hashes: Vec<BytesN<32>>,
) -> Vec<ProjectStatusEnum> {
    let mut result = Vec::new(e);
    for project_hash in project_hashes {
        let project_key = DataKey::ProjectStatus(project_hash.clone());
        let project_status = e.storage().instance().get(&project_key).unwrap_or(ProjectStatusEnum::NotSet);
        result.push_back(project_status);
    }
    result
}

pub fn set_trufa_score(e: &Env, project_hash: &BytesN<32>, score: &TrufaScoreValues) {
    let key = DataKey::TrufaScore(project_hash.clone());
    e.storage().instance().set(&key, score);
}

pub fn get_trufa_score(e: &Env, project_hash: &BytesN<32>) -> TrufaScoreValues {
    let key = DataKey::TrufaScore(project_hash.clone());
    e.storage().instance().get(&key).unwrap()
}