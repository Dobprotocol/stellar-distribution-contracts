//! This contract demonstrates a sample implementation of the Soroban token
//! interface.
use crate::admin::{read_administrator, write_administrator};
use crate::whitelist;
use crate::project;
use crate::storage_types::{ProjectStatusEnum, ProjectData, TrufaScoreValues};
use crate::storage_types::{INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD};
use soroban_sdk::{contract, contractimpl, Address, Env, Vec, BytesN};


#[contract]
pub struct Projects;

#[contractimpl]
impl Projects {
    pub fn __constructor(e: Env, admin: Address, whitelist_addresses: Vec<Address>) {
        write_administrator(&e, &admin);
        
        // extend the instance lifetime
        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        for address in whitelist_addresses {
            whitelist::add_to_whitelist(&e, &address);
            //emit event
            e.events().publish(
                ("whitelist", "added"),
                (address,),
            )
        };
    }

    pub fn is_whitelisted(e: Env, address: Address) -> bool {

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        whitelist::is_whitelisted(&e, &address)
    }

    pub fn add_to_whitelist(e: Env, address: Address) {
        // only admins can do this
        let admin = read_administrator(&e);
        admin.require_auth();

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        whitelist::add_to_whitelist(&e, &address);

        //emit event
        e.events().publish(
            ("whitelist", "added"),
            (address,),
        )
    }

    pub fn remove_from_whitelist(e: Env, address: Address) {
        // only admins can do this
        let admin = read_administrator(&e);
        admin.require_auth();

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        whitelist::remove_from_whitelist(&e, &address);

        // emit event
        e.events().publish(
            ("whitelist", "removed"),
            (address,),
        );
    }

    pub fn add_project(e: Env, from: Address, project_hash: BytesN<32>) {
        // check authorization
        from.require_auth();

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        // check that project does not exists
        let status = project::get_project_status(&e, &project_hash);
        if status != ProjectStatusEnum::NotSet {
            // throw an error
            panic!("Project already exists");
        }
        // add the project in Pending status
        project::set_project_status(&e, &project_hash, &ProjectStatusEnum::Pending);
        // emit event
        e.events().publish(
            ("project", "added"),
            (from, project_hash.clone(), ProjectStatusEnum::Pending),
        );
    }

    pub fn set_project_approved(e: Env, from: Address, project_hash: BytesN<32>, trufa_score_values: TrufaScoreValues) {
        // check authorization
        from.require_auth();

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        // check that this address is whitelisted
        let ok = whitelist::is_whitelisted(&e, &from);
        if !ok {
            // throw an error
            panic!("Address is not whitelisted");
        }
        // check that project is pending
        let status = project::get_project_status(&e, &project_hash);
        if status != ProjectStatusEnum::Pending{
            // throw an error
            panic!("Project is not pending");
        }
        // set the project status to approved
        project::set_project_status(&e, &project_hash, &ProjectStatusEnum::Approved);

        // set the project trufa score values
        project::set_trufa_score(&e, &project_hash, &trufa_score_values);

        // emit event
        e.events().publish(
            ("project", "approved"),
            (from, project_hash.clone(), ProjectStatusEnum::Approved),
        );
    }

    pub fn set_project_rejected(e: Env, from: Address, project_hash: BytesN<32>) {
        // get the current addess that executed this function
        from.require_auth();

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        // check that this address is whitelisted
        let ok = whitelist::is_whitelisted(&e, &from);
        if !ok {
            // throw an error
            panic!("Address is not whitelisted");
        }
        // check that project is pending
        let status = project::get_project_status(&e, &project_hash);
        if status != ProjectStatusEnum::Pending {
            // throw an error
            panic!("Project is not pending");
        }
        // set the project status to rejected
        project::set_project_status(&e, &project_hash, &ProjectStatusEnum::Rejected);
        // emit event
        e.events().publish(
            ("project", "rejected"),
            (from, project_hash.clone(), ProjectStatusEnum::Rejected),
        );
    }

    pub fn reset_project(e: Env, project_hash: BytesN<32>) {
        // only admin can do this
        let admin = read_administrator(&e);
        admin.require_auth();

        // only Rejected projects can be reset
        let status = project::get_project_status(&e, &project_hash);
        if status != ProjectStatusEnum::Rejected {
            // throw an error
            panic!("Project is not rejected");
        }

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        // set project status to Pending
        project::set_project_status(&e, &project_hash, &ProjectStatusEnum::Pending);
    }

    pub fn get_project_status(e: Env, project_hash: BytesN<32>) -> ProjectStatusEnum {

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        project::get_project_status(&e, &project_hash)
    }

    pub fn get_projects_statuses_in_bulk(e: Env, start: u32, end: u32) -> Vec<ProjectData> {

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        project::get_projects_statuses_in_bulk(&e, start, end)
    }

    pub fn get_projects_statuses_from_vec(e: Env, project_hashes: Vec<BytesN<32>>) -> Vec<ProjectStatusEnum> {

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        project::get_projects_statuses_from_vec(&e, project_hashes)
    }

    pub fn get_all_projects_statuses(e: Env) -> Vec<ProjectData> {

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        let end = project::get_number_of_projects(&e);
        project::get_projects_statuses_in_bulk(&e, 0, end)
    }

    pub fn set_admin(e: Env, new_admin: Address) {
        let admin = read_administrator(&e);
        admin.require_auth();

        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        write_administrator(&e, &new_admin);
        // add event
        e.events().publish(
            ("admin", "updated"),
            (admin, new_admin),
        );
    }

    pub fn get_trufa_score(e: Env, project_hash: BytesN<32>) -> TrufaScoreValues {
        // check that project was approved
        let status = project::get_project_status(&e, &project_hash);
        if status != ProjectStatusEnum::Approved {
            // throw an error
            panic!("Project is not approved");
        }
        // extend the instance lifetime
        e.storage()
            .instance()
            .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);

        project::get_trufa_score(&e, &project_hash)
    }
}
