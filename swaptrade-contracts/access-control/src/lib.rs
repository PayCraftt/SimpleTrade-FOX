#![no_std]

use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short,
    Address, Env, Symbol, Vec, Map,
};

const OWNER: Symbol = symbol_short!("OWNER");
const ADMIN: Symbol = symbol_short!("ADMIN");
const TRADER: Symbol = symbol_short!("TRADER");
const VIEWER: Symbol = symbol_short!("VIEWER");

#[contracttype]
pub enum Role {
    Owner,
    Admin,
    Trader,
    Viewer,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AccessControlError {
    NotAuthorized = 1,
    RoleAlreadyAssigned = 2,
    RoleNotFound = 3,
    CannotRevokeOwner = 4,
    CannotRenounceOwner = 5,
    InvalidRole = 6,
}

#[contracttype]
pub struct RoleData {
    pub role: Role,
    pub assigned_at: u64,
    pub assigned_by: Address,
}

#[contract]
pub struct AccessControlContract {
    /// Map<Address, Map<Symbol, RoleData>>
    roles: Map<Address, Map<Symbol, RoleData>>,
    /// Contract owner (can grant/revoke any role)
    owner: Address,
}

#[contractimpl]
impl AccessControlContract {
    /// Initialize the access control contract with an owner
    pub fn initialize(env: Env, owner: Address) -> Result<(), AccessControlError> {
        if env.storage().instance().has(&symbol_short!("INIT")) {
            return Err(AccessControlError::RoleAlreadyAssigned);
        }

        let roles: Map<Address, Map<Symbol, RoleData>> = env.storage().instance().get(&symbol_short!("ROLES")).unwrap_or_else(|| {
            Map::new(&env)
        });

        let role_data = RoleData {
            role: Role::Owner,
            assigned_at: env.ledger().timestamp(),
            assigned_by: owner.clone(),
        };

        let mut user_roles = Map::new(&env);
        user_roles.set(OWNER.clone(), role_data);
        roles.set(owner.clone(), user_roles);

        env.storage().instance().set(&symbol_short!("OWNER"), &owner);
        env.storage().instance().set(&symbol_short!("INIT"), &true);
        env.storage().instance().set(&symbol_short!("ROLES"), &roles);

        Ok(())
    }

    /// Grant a role to an address (requires Owner or Admin role)
    pub fn grant_role(
        env: Env,
        caller: Address,
        user: Address,
        role: Role,
    ) -> Result<(), AccessControlError> {
        caller.require_auth();

        if !Self::has_role_internal(&env, &caller, &Role::Owner) &&
           !Self::has_role_internal(&env, &caller, &Role::Admin) {
            return Err(AccessControlError::NotAuthorized);
        }

        // Only Owner can grant Owner or Admin roles
        if matches!(role, Role::Owner | Role::Admin) {
            if !Self::has_role_internal(&env, &caller, &Role::Owner) {
                return Err(AccessControlError::NotAuthorized);
            }
        }

        Self::grant_role_internal(&env, &user, &role, &caller)
    }

    /// Revoke a role from an address (requires Owner or Admin role)
    pub fn revoke_role(
        env: Env,
        caller: Address,
        user: Address,
        role: Role,
    ) -> Result<(), AccessControlError> {
        caller.require_auth();

        if !Self::has_role_internal(&env, &caller, &Role::Owner) &&
           !Self::has_role_internal(&env, &caller, &Role::Admin) {
            return Err(AccessControlError::NotAuthorized);
        }

        // Only Owner can revoke Owner or Admin roles
        if matches!(role, Role::Owner | Role::Admin) {
            if !Self::has_role_internal(&env, &caller, &Role::Owner) {
                return Err(AccessControlError::NotAuthorized);
            }
        }

        // Cannot revoke Owner role
        if matches!(role, Role::Owner) {
            return Err(AccessControlError::CannotRevokeOwner);
        }

        Self::revoke_role_internal(&env, &user, &role)
    }

    /// Check if an address has a specific role
    pub fn has_role(env: Env, user: Address, role: Role) -> bool {
        Self::has_role_internal(&env, &user, &role)
    }

    /// Get all roles for an address
    pub fn get_user_roles(env: Env, user: Address) -> Vec<Symbol> {
        let roles: Map<Address, Map<Symbol, RoleData>> = env.storage().instance().get(&symbol_short!("ROLES")).unwrap_or_else(|| {
            Map::new(&env)
        });

        let mut result = Vec::new(&env);

        if let Some(user_roles) = roles.get(user) {
            if user_roles.contains(OWNER.clone()) {
                result.push_back(OWNER.clone());
            }
            if user_roles.contains(ADMIN.clone()) {
                result.push_back(ADMIN.clone());
            }
            if user_roles.contains(TRADER.clone()) {
                result.push_back(TRADER.clone());
            }
            if user_roles.contains(VIEWER.clone()) {
                result.push_back(VIEWER.clone());
            }
        }

        result
    }

    /// Get role details for a specific user and role
    pub fn get_role_data(env: Env, user: Address, role: Symbol) -> Option<RoleData> {
        let roles: Map<Address, Map<Symbol, RoleData>> = env.storage().instance().get(&symbol_short!("ROLES")).unwrap_or_else(|| {
            Map::new(&env)
        });

        roles.get(user).and_then(|user_roles| user_roles.get(role))
    }

    /// Transfer ownership to a new address (requires Owner role)
    pub fn transfer_ownership(
        env: Env,
        caller: Address,
        new_owner: Address,
    ) -> Result<(), AccessControlError> {
        caller.require_auth();

        if !Self::has_role_internal(&env, &caller, &Role::Owner) {
            return Err(AccessControlError::NotAuthorized);
        }

        // Grant Owner role to new owner
        Self::grant_role_internal(&env, &new_owner, &Role::Owner, &caller)?;

        // Revoke Owner role from current owner
        Self::revoke_role_internal(&env, &caller, &Role::Owner)?;

        // Update owner reference
        env.storage().instance().set(&symbol_short!("OWNER"), &new_owner);

        Ok(())
    }

    // Internal helpers
    fn has_role_internal(env: &Env, user: &Address, role: &Role) -> bool {
        let roles: Map<Address, Map<Symbol, RoleData>> = env.storage().instance().get(&symbol_short!("ROLES")).unwrap_or_else(|| {
            Map::new(env)
        });

        let symbol = match role {
            Role::Owner => OWNER.clone(),
            Role::Admin => ADMIN.clone(),
            Role::Trader => TRADER.clone(),
            Role::Viewer => VIEWER.clone(),
        };

        roles.get(user.clone()).map_or(false, |user_roles| {
            user_roles.contains(symbol)
        })
    }

    fn grant_role_internal(
        env: &Env,
        user: &Address,
        role: &Role,
        granted_by: &Address,
    ) -> Result<(), AccessControlError> {
        let mut roles: Map<Address, Map<Symbol, RoleData>> = env.storage().instance().get(&symbol_short!("ROLES")).unwrap_or_else(|| {
            Map::new(env)
        });

        let symbol = match role {
            Role::Owner => OWNER.clone(),
            Role::Admin => ADMIN.clone(),
            Role::Trader => TRADER.clone(),
            Role::Viewer => VIEWER.clone(),
        };

        let role_data = RoleData {
            role: role.clone(),
            assigned_at: env.ledger().timestamp(),
            assigned_by: granted_by.clone(),
        };

        let mut user_roles = roles.get(user.clone()).unwrap_or_else(|| {
            Map::new(env)
        });

        user_roles.set(symbol, role_data);
        roles.set(user.clone(), user_roles);

        env.storage().instance().set(&symbol_short!("ROLES"), &roles);

        Ok(())
    }

    fn revoke_role_internal(
        env: &Env,
        user: &Address,
        role: &Role,
    ) -> Result<(), AccessControlError> {
        let mut roles: Map<Address, Map<Symbol, RoleData>> = env.storage().instance().get(&symbol_short!("ROLES")).unwrap_or_else(|| {
            Map::new(env)
        });

        let symbol = match role {
            Role::Owner => OWNER.clone(),
            Role::Admin => ADMIN.clone(),
            Role::Trader => TRADER.clone(),
            Role::Viewer => VIEWER.clone(),
        };

        if let Some(mut user_roles) = roles.get(user.clone()) {
            if user_roles.contains(symbol.clone()) {
                user_roles.remove(symbol);
                roles.set(user.clone(), user_roles);
                env.storage().instance().set(&symbol_short!("ROLES"), &roles);
                Ok(())
            } else {
                Err(AccessControlError::RoleNotFound)
            }
        } else {
            Err(AccessControlError::RoleNotFound)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;

    fn setup() -> (Env, Address, AccessControlContract) {
        let env = Env::default();
        let owner = Address::generate(&env);
        let contract = AccessControlContract;
        contract.initialize(env.clone(), owner.clone()).unwrap();
        (env, owner, contract)
    }

    #[test]
    fn test_initialize() {
        let (env, owner, contract) = setup();
        assert!(contract.has_role(env.clone(), owner, Role::Owner));
    }

    #[test]
    fn test_grant_role() {
        let (env, owner, contract) = setup();
        let user = Address::generate(&env);
        
        env.mock_auths(&[
            soroban_sdk::testutils::MockAuth {
                address: &owner,
                invoke: &soroban_sdk::testutils::MockAuthInvoke {
                    contract: &env.register_contract(None, AccessControlContract),
                    fn_name: "grant_role",
                    args: (&user, Role::Trader).into_val(&env),
                    sub_invokes: &[],
                },
            },
        ]);

        contract.grant_role(env.clone(), owner.clone(), user.clone(), Role::Trader).unwrap();
        assert!(contract.has_role(env.clone(), user, Role::Trader));
    }

    #[test]
    fn test_revoke_role() {
        let (env, owner, contract) = setup();
        let user = Address::generate(&env);
        
        env.mock_auths(&[
            soroban_sdk::testutils::MockAuth {
                address: &owner,
                invoke: &soroban_sdk::testutils::MockAuthInvoke {
                    contract: &env.register_contract(None, AccessControlContract),
                    fn_name: "grant_role",
                    args: (&user, Role::Trader).into_val(&env),
                    sub_invokes: &[],
                },
            },
        ]);

        contract.grant_role(env.clone(), owner.clone(), user.clone(), Role::Trader).unwrap();
        
        env.mock_auths(&[
            soroban_sdk::testutils::MockAuth {
                address: &owner,
                invoke: &soroban_sdk::testutils::MockAuthInvoke {
                    contract: &env.register_contract(None, AccessControlContract),
                    fn_name: "revoke_role",
                    args: (&user, Role::Trader).into_val(&env),
                    sub_invokes: &[],
                },
            },
        ]);

        contract.revoke_role(env.clone(), owner.clone(), user.clone(), Role::Trader).unwrap();
        assert!(!contract.has_role(env.clone(), user, Role::Trader));
    }

    #[test]
    fn test_cannot_revoke_owner() {
        let (env, owner, contract) = setup();
        
        env.mock_auths(&[
            soroban_sdk::testutils::MockAuth {
                address: &owner,
                invoke: &soroban_sdk::testutils::MockAuthInvoke {
                    contract: &env.register_contract(None, AccessControlContract),
                    fn_name: "revoke_role",
                    args: (&owner, Role::Owner).into_val(&env),
                    sub_invokes: &[],
                },
            },
        ]);

        let result = contract.revoke_role(env.clone(), owner.clone(), owner.clone(), Role::Owner);
        assert_eq!(result, Err(AccessControlError::CannotRevokeOwner));
    }

    #[test]
    fn test_unauthorized_grant() {
        let (env, _owner, contract) = setup();
        let unauthorized = Address::generate(&env);
        let user = Address::generate(&env);
        
        env.mock_auths(&[
            soroban_sdk::testutils::MockAuth {
                address: &unauthorized,
                invoke: &soroban_sdk::testutils::MockAuthInvoke {
                    contract: &env.register_contract(None, AccessControlContract),
                    fn_name: "grant_role",
                    args: (&user, Role::Trader).into_val(&env),
                    sub_invokes: &[],
                },
            },
        ]);

        let result = contract.grant_role(env.clone(), unauthorized, user, Role::Trader);
        assert_eq!(result, Err(AccessControlError::NotAuthorized));
    }
}
