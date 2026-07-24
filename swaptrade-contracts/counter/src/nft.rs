#![cfg(feature = "nft")]

use soroban_sdk::{contract, contractimpl, Address, Env, String, Symbol};

#[contract]
pub struct NonFungibleToken;

#[contractimpl]
impl NonFungibleToken {
    pub fn name(env: Env) -> String {
        String::from_str(&env, "Achievement NFT")
    }

    pub fn symbol(env: Env) -> String {
        String::from_str(&env, "ANFT")
    }

    pub fn mint(env: Env, to: Address, token_uri: String) {
        // For simplicity, we'll use the token_uri as the token_id
        let token_id = token_uri;
        let owner_key = (Symbol::new(&env, "owner"), token_id.clone());
        if env.storage().persistent().has(&owner_key) {
            panic!("token already minted");
        }
        env.storage().persistent().set(&owner_key, &to);

        let balance_key = (Symbol::new(&env, "balance"), to.clone());
        let balance: u32 = env.storage().persistent().get(&balance_key).unwrap_or(0);
        env.storage().persistent().set(&balance_key, &(balance + 1));
    }

    pub fn owner_of(env: Env, token_id: String) -> Address {
        let owner_key = (Symbol::new(&env, "owner"), token_id);
        env.storage().persistent().get(&owner_key).unwrap()
    }

    pub fn balance_of(env: Env, owner: Address) -> u32 {
        let balance_key = (Symbol::new(&env, "balance"), owner);
        env.storage().persistent().get(&balance_key).unwrap_or(0)
    }
}