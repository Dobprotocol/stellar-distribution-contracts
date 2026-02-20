#![cfg(test)]
extern crate std;
use crate::{contract::Projects, ProjectsClient};
use crate::storage_types::{ProjectStatusEnum, TrufaScoreValues};
use soroban_sdk::{
    testutils::{Address as _, AuthorizedFunction, AuthorizedInvocation},
    Address, Env, IntoVal, Symbol, BytesN, Vec, vec
};

// add function to create addresses vector
fn create_addresses(e: &Env, &n: &u32) -> Vec<Address> {
    let mut result = Vec::new(e);
    for _i in 0..n {
        result.push_front(Address::generate(e));
    }
    result
}

// add function to instanciate the contract
fn create_projects_contract<'a>(e: &Env, admin: &Address, whitelist_addresses: Vec<Address>) -> ProjectsClient<'a> {
    let project_id = e.register(Projects, (admin, whitelist_addresses));
    ProjectsClient::new(e, &project_id)
}

#[test]
fn test_whitelist() {
    let env = Env::default();
    env.mock_all_auths();

    let addresses = create_addresses(&env, &10);

    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address

    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());

    for address in whitelist_addresses {
        let is_whitelisted = client.is_whitelisted(&address);
        assert_eq!(is_whitelisted, true);
    }

    // add a new whitelisted address
    // first check that this address is not whitelisted
    let is_whitelisted = client.is_whitelisted(&addresses.get(6).unwrap());
    assert_eq!(is_whitelisted, false);
    // add the address
    client.add_to_whitelist(&addresses.get(5).unwrap());
    assert_eq!(
        env.auths(),
        std::vec![(
            admin.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "add_to_whitelist"),
                    (&addresses.get(5).unwrap(),).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that this address is now whitelisted
    let is_whitelisted = client.is_whitelisted(&addresses.get(5).unwrap());
    assert_eq!(is_whitelisted, true);

    // try to add an address already whitelisted will just update its status in the key:value mapping
    client.add_to_whitelist(&addresses.get(5).unwrap());
    assert_eq!(
        env.auths(),
        std::vec![(
            admin.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "add_to_whitelist"),
                    (&addresses.get(5).unwrap(),).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that this address is now whitelisted
    let is_whitelisted = client.is_whitelisted(&addresses.get(5).unwrap());
    assert_eq!(is_whitelisted, true);
}

#[test]
fn test_add_project_and_approve() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    let trufa_scores = TrufaScoreValues {
        technical_feasibility: 70,
        regulatory_compliance: 75,
        financial_viability: 80,
        environment_impact: 75,
        overall_trufa_score: 75
    };

    // check project status before is notSet
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::NotSet);
    // add project
    client.add_project(&user1, &project1_hash);
    assert_eq!(
        env.auths(),
        std::vec![(
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "add_project"),
                    (&user1, &project1_hash).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that project status is now Pending
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Pending);
    // approve the project used a whitelisted address
    client.set_project_approved(&whitelist_addresses.get(1).unwrap(), &project1_hash, &trufa_scores);
    assert_eq!(
        env.auths(),
        std::vec![(
            whitelist_addresses.get(1).unwrap().clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "set_project_approved"),
                    (&whitelist_addresses.get(1).unwrap(), &project1_hash, trufa_scores.clone()).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that project status is now Approved
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Approved);
}

#[test]
fn test_add_project_and_reject() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);

    // check project status before is notSet
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::NotSet);
    // add project
    client.add_project(&user1, &project1_hash);
    assert_eq!(
        env.auths(),
        std::vec![(
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "add_project"),
                    (&user1, &project1_hash).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that project status is now Pending
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Pending);
    // reject the project used a whitelisted address
    client.set_project_rejected(&whitelist_addresses.get(1).unwrap(), &project1_hash);
    assert_eq!(
        env.auths(),
        std::vec![(
            whitelist_addresses.get(1).unwrap().clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "set_project_rejected"),
                    (&whitelist_addresses.get(1).unwrap(), &project1_hash).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that project status is now Rejected
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Rejected);
}

#[test]
#[should_panic(expected = "Address is not whitelisted")]
fn test_add_project_and_failed_to_approve() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    let trufa_scores = TrufaScoreValues {
        technical_feasibility: 70,
        regulatory_compliance: 75,
        financial_viability: 80,
        environment_impact: 75,
        overall_trufa_score: 75
    };

    // check project status before is notSet
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::NotSet);
    // add project
    client.add_project(&user1, &project1_hash);
    assert_eq!(
        env.auths(),
        std::vec![(
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "add_project"),
                    (&user1, &project1_hash).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that project status is now Pending
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Pending);
    // try to approve with non-whitelisted address should panic
    client.set_project_approved(&addresses.get(7).unwrap(), &project1_hash, &trufa_scores);
}

#[test]
#[should_panic(expected = "Address is not whitelisted")]
fn test_add_project_and_failed_to_reject() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    // check project status before is notSet
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::NotSet);
    // add project
    client.add_project(&user1, &project1_hash);
    assert_eq!(
        env.auths(),
        std::vec![(
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "add_project"),
                    (&user1, &project1_hash).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that project status is now Pending
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Pending);
    // try to reject with non-whitelisted address should panic
    client.set_project_rejected(&addresses.get(7).unwrap(), &project1_hash);
}

#[test]
#[should_panic(expected = "Project is not pending")]
fn test_approve_of_not_set_project() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    let trufa_scores = TrufaScoreValues {
        technical_feasibility: 70,
        regulatory_compliance: 75,
        financial_viability: 80,
        environment_impact: 75,
        overall_trufa_score: 75
    };
    // try to approve not-set project should panic
    client.set_project_approved(&whitelist_addresses.get(0).unwrap(), &project1_hash, &trufa_scores);
}

#[test]
#[should_panic(expected = "Project is not pending")]
fn test_reject_of_not_set_project() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());

    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    // try to reject not-set project should panic
    client.set_project_rejected(&whitelist_addresses.get(0).unwrap(), &project1_hash);
}

#[test]
#[should_panic(expected = "Project already exists")]
fn test_add_project_twice() {
    let env = Env::default();
    env.mock_all_auths();

    let addresses = create_addresses(&env, &10);

    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address

    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());

    let project1_hash = BytesN::from_array(&env, &[1; 32]);

    // add project twice
    client.add_project(&user1, &project1_hash);
    client.add_project(&user1, &project1_hash);
}

#[test]
#[should_panic(expected = "Project already exists")]
fn test_add_project_already_rejected() {
    let env = Env::default();
    env.mock_all_auths();

    let addresses = create_addresses(&env, &10);

    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address

    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());

    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    // check project status before is notSet
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::NotSet);
    // add project
    client.add_project(&user1, &project1_hash);
    // check that project status is now Pending
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Pending);
    // reject the project used a whitelisted address
    client.set_project_rejected(&whitelist_addresses.get(1).unwrap(), &project1_hash);
    // check that project status is now Rejected
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Rejected);
    // add project again should panic
    client.add_project(&user1, &project1_hash);
}

#[test]
#[should_panic(expected = "Project already exists")]
fn test_add_project_already_accepted() {
    let env = Env::default();
    env.mock_all_auths();

    let addresses = create_addresses(&env, &10);

    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address

    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());

    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    let trufa_scores = TrufaScoreValues {
        technical_feasibility: 70,
        regulatory_compliance: 75,
        financial_viability: 80,
        environment_impact: 75,
        overall_trufa_score: 75
    };
    // check project status before is notSet
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::NotSet);
    // add project
    client.add_project(&user1, &project1_hash);
    // check that project status is now Pending
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Pending);
    // accept the project used a whitelisted address
    client.set_project_approved(&whitelist_addresses.get(1).unwrap(), &project1_hash, &trufa_scores);
    // check that project status is now Accepted
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Approved);
    // add project again should panic
    client.add_project(&user1, &project1_hash);
}

#[test]
fn test_reset_a_rejected_project() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);

    // add project
    client.add_project(&user1, &project1_hash);
    assert_eq!(
        env.auths(),
        std::vec![(
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "add_project"),
                    (&user1, &project1_hash).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // reject the project used a whitelisted address
    client.set_project_rejected(&whitelist_addresses.get(1).unwrap(), &project1_hash);
    assert_eq!(
        env.auths(),
        std::vec![(
            whitelist_addresses.get(1).unwrap().clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "set_project_rejected"),
                    (&whitelist_addresses.get(1).unwrap(), &project1_hash).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that project status is now Rejected
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Rejected);
    // reset the project
    client.reset_project(&project1_hash);
    assert_eq!(
        env.auths(),
        std::vec![(
            admin.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "reset_project"),
                    (&project1_hash,).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that project status is now Pending
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Pending);
}

#[test]
#[should_panic(expected = "Project is not rejected")]
fn test_reset_an_approved_project() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    let trufa_scores = TrufaScoreValues {
        technical_feasibility: 70,
        regulatory_compliance: 75,
        financial_viability: 80,
        environment_impact: 75,
        overall_trufa_score: 75
    };

    // add project
    client.add_project(&user1, &project1_hash);
    // approve the project used a whitelisted address
    client.set_project_approved(&whitelist_addresses.get(1).unwrap(), &project1_hash, &trufa_scores);
    // check that project status is now Approved
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Approved);
    // reset the project should panic
    client.reset_project(&project1_hash);
}

#[test]
fn test_add_project_reject_reset_and_approve() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    let trufa_scores = TrufaScoreValues {
        technical_feasibility: 70,
        regulatory_compliance: 75,
        financial_viability: 80,
        environment_impact: 75,
        overall_trufa_score: 75
    };

    // add project
    client.add_project(&user1, &project1_hash);
    assert_eq!(
        env.auths(),
        std::vec![(
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "add_project"),
                    (&user1, &project1_hash).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // reject the project used a whitelisted address
    client.set_project_rejected(&whitelist_addresses.get(1).unwrap(), &project1_hash);
    assert_eq!(
        env.auths(),
        std::vec![(
            whitelist_addresses.get(1).unwrap(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "set_project_rejected"),
                    (&whitelist_addresses.get(1).unwrap(), &project1_hash).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that project status is now Rejected
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Rejected);
    // reset the project
    client.reset_project(&project1_hash);
    assert_eq!(
        env.auths(),
        std::vec![(
            admin.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "reset_project"),
                    (&project1_hash,).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that project status is now Pending
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Pending);
    // approve the project used a whitelisted address
    client.set_project_approved(&whitelist_addresses.get(1).unwrap(), &project1_hash, &trufa_scores);
    assert_eq!(
        env.auths(),
        std::vec![(
            whitelist_addresses.get(1).unwrap(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    client.address.clone(),
                    Symbol::new(&env, "set_project_approved"),
                    (&whitelist_addresses.get(1).unwrap(), &project1_hash, trufa_scores.clone()).into_val(&env),
                )),
                sub_invocations: std::vec![]
            }
        )]
    );
    // check that project status is now Approved
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Approved);
}

#[test]
fn test_add_two_different_project_approve_one_and_reject_other () {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    let project2_hash = BytesN::from_array(&env, &[2; 32]);
    let trufa_scores = TrufaScoreValues {
        technical_feasibility: 30,
        regulatory_compliance: 40,
        financial_viability: 50,
        environment_impact: 40,
        overall_trufa_score: 40
    };

    // add project 1 and 2
    client.add_project(&user1, &project1_hash);
    client.add_project(&user1, &project2_hash);
    // reject the project 1 used a whitelisted address
    client.set_project_rejected(&whitelist_addresses.get(1).unwrap(), &project1_hash);
    // check that project 1 status is now Rejected
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Rejected);
    // check that project 2 status is now Pending
    let status = client.get_project_status(&project2_hash);
    assert_eq!(status, ProjectStatusEnum::Pending);
    // approve the project 2 used a whitelisted address
    client.set_project_approved(&whitelist_addresses.get(1).unwrap(), &project2_hash, &trufa_scores);
    // check that project 1 status is still Rejected
    let status = client.get_project_status(&project1_hash);
    assert_eq!(status, ProjectStatusEnum::Rejected);
    // check that project 2 status is now Approved
    let status = client.get_project_status(&project2_hash);
    assert_eq!(status, ProjectStatusEnum::Approved);
}

#[test]
fn test_get_all_project_statuses() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let user2 = addresses.get(6).unwrap(); // takes 7th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    let project2_hash = BytesN::from_array(&env, &[2; 32]);
    let project3_hash = BytesN::from_array(&env, &[3; 32]);
    let trufa_scores = TrufaScoreValues {
        technical_feasibility: 30,
        regulatory_compliance: 40,
        financial_viability: 50,
        environment_impact: 40,
        overall_trufa_score: 40
    };

    // add projects 1 and 2 using user1
    client.add_project(&user1, &project1_hash);
    client.add_project(&user1, &project2_hash);
    // add project 3 using user2
    client.add_project(&user2, &project3_hash);

    // set project 2 to approved
    client.set_project_approved(&whitelist_addresses.get(1).unwrap(), &project2_hash, &trufa_scores);
    // set project 3 to rejected
    client.set_project_rejected(&whitelist_addresses.get(2).unwrap(), &project3_hash);
    // check that all projects statuses are returned
    let status = client.get_all_projects_statuses();
    assert_eq!(status.len(), 3);
    assert_eq!(status.get(0).unwrap().hash, project1_hash);
    assert_eq!(status.get(0).unwrap().status, ProjectStatusEnum::Pending);
    assert_eq!(status.get(1).unwrap().hash, project2_hash);
    assert_eq!(status.get(1).unwrap().status, ProjectStatusEnum::Approved);
    assert_eq!(status.get(2).unwrap().hash, project3_hash);
    assert_eq!(status.get(2).unwrap().status, ProjectStatusEnum::Rejected);
}

#[test]
fn test_get_project_statuses_from_vec() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let user2 = addresses.get(6).unwrap(); // takes 7th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    let project2_hash = BytesN::from_array(&env, &[2; 32]);
    let project3_hash = BytesN::from_array(&env, &[3; 32]);
    let project4_hash = BytesN::from_array(&env, &[4; 32]);
    let trufa_scores = TrufaScoreValues {
        technical_feasibility: 30,
        regulatory_compliance: 40,
        financial_viability: 50,
        environment_impact: 40,
        overall_trufa_score: 40
    };

    // add all projects using any user, is not relevant at all
    client.add_project(&user1, &project1_hash);
    client.add_project(&user1, &project2_hash);
    client.add_project(&user2, &project3_hash);
    client.add_project(&user1, &project4_hash);

    // set project 3 to rejected and project 1 to approved
    client.set_project_approved(&whitelist_addresses.get(1).unwrap(), &project1_hash, &trufa_scores);
    client.set_project_rejected(&whitelist_addresses.get(2).unwrap(), &project3_hash);
    // get the status for projects 1,3 and 4
    let query_vec = vec![&env, project1_hash.clone(), project3_hash.clone(), project4_hash.clone()];
    let status = client.get_projects_statuses_from_vec(&query_vec);
    assert_eq!(status.len(), 3);
    assert_eq!(status.get(0).unwrap(), ProjectStatusEnum::Approved);
    assert_eq!(status.get(1).unwrap(), ProjectStatusEnum::Rejected);
    assert_eq!(status.get(2).unwrap(), ProjectStatusEnum::Pending);

    // get a different sub-vector, now take projects 2 and 3
    let query_vec = vec![&env, project2_hash.clone(), project3_hash.clone()];
    let status = client.get_projects_statuses_from_vec(&query_vec);
    assert_eq!(status.len(), 2);
    assert_eq!(status.get(0).unwrap(), ProjectStatusEnum::Pending);
    assert_eq!(status.get(1).unwrap(), ProjectStatusEnum::Rejected);
}

#[test]
fn test_get_projects_in_bulk() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let user2 = addresses.get(6).unwrap(); // takes 7th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);
    let project2_hash = BytesN::from_array(&env, &[2; 32]);
    let project3_hash = BytesN::from_array(&env, &[3; 32]);
    let project4_hash = BytesN::from_array(&env, &[4; 32]);
    let trufa_scores = TrufaScoreValues {
        technical_feasibility: 30,
        regulatory_compliance: 40,
        financial_viability: 50,
        environment_impact: 40,
        overall_trufa_score: 40
    };
    let trufa_scores2 = TrufaScoreValues {
        technical_feasibility: 76,
        regulatory_compliance: 74,
        financial_viability: 21,
        environment_impact: 87,
        overall_trufa_score: 65
    };

    // add projects 1 and 2 using user1
    client.add_project(&user1, &project1_hash);
    client.add_project(&user1, &project2_hash);
    // add project 3 and 4 using user2
    client.add_project(&user2, &project3_hash);
    client.add_project(&user2, &project4_hash);

    // set project 2 to approved
    client.set_project_approved(&whitelist_addresses.get(1).unwrap(), &project2_hash, &trufa_scores);
    // set project 3 to rejected
    client.set_project_rejected(&whitelist_addresses.get(2).unwrap(), &project3_hash);
    // set project 4 to approved
    client.set_project_approved(&whitelist_addresses.get(1).unwrap(), &project4_hash, &trufa_scores2);
    // get the sub-vec from idx 1 to idx 10 (the code will just return the available number of projects)
    // this basically skips the first project
    let status = client.get_projects_statuses_in_bulk(&1, &10);
    assert_eq!(status.len(), 3);
    assert_eq!(status.get(0).unwrap().hash, project2_hash);
    assert_eq!(status.get(0).unwrap().status, ProjectStatusEnum::Approved);
    assert_eq!(status.get(1).unwrap().hash, project3_hash);
    assert_eq!(status.get(1).unwrap().status, ProjectStatusEnum::Rejected);
    assert_eq!(status.get(2).unwrap().hash, project4_hash);
    assert_eq!(status.get(2).unwrap().status, ProjectStatusEnum::Approved);

    // check that trufa score is correctly saved
    let trufa_score = client.get_trufa_score(&project2_hash);
    assert_eq!(trufa_score.technical_feasibility, 30);
    assert_eq!(trufa_score.regulatory_compliance, 40);
    assert_eq!(trufa_score.financial_viability, 50);
    assert_eq!(trufa_score.environment_impact, 40);
    assert_eq!(trufa_score.overall_trufa_score, 40);

    let trufa_score = client.get_trufa_score(&project4_hash);
    assert_eq!(trufa_score.technical_feasibility, 76);
    assert_eq!(trufa_score.regulatory_compliance, 74);
    assert_eq!(trufa_score.financial_viability, 21);
    assert_eq!(trufa_score.environment_impact, 87);
    assert_eq!(trufa_score.overall_trufa_score, 65);
}

#[test]
#[should_panic(expected = "Project is not approved")]
fn test_get_trufa_score_of_not_approved_project() {
    let env = Env::default();
    env.mock_all_auths();
    let addresses = create_addresses(&env, &10);
    let admin = addresses.get(0).unwrap(); // takes 1st address
    let whitelist_addresses = addresses.slice(1..5); // takes from 2nd to 5th address
    let user1 = addresses.get(5).unwrap(); // takes 6th address
    let client = create_projects_contract(&env, &admin, whitelist_addresses.clone());
    let project1_hash = BytesN::from_array(&env, &[1; 32]);

    // add projects 1 and 2 using user1
    client.add_project(&user1, &project1_hash);

    // get trufa score of pending project should panic
    client.get_trufa_score(&project1_hash);

}
