
#![cfg(feature = "nft")]

use soroban_sdk::{contract, contractimpl, Address, Env, Symbol};
use crate::nft_types::{Achievement, get_nft_metadata};
use soroban_nft::minting::{self, NonFungibleTokenMinting};

#[contract]
pub struct NftMintingContract;

#[contractimpl]
impl NftMintingContract {
    pub fn mint_achievement_nft(env: Env, to: Address, achievement: Achievement) {
        let mut portfolio: crate::portfolio::Portfolio = env.storage().instance().get(&()).unwrap_or_else(|| crate::portfolio::Portfolio::new(&env));

        if portfolio.has_minted_achievement(&env, to.clone(), achievement.clone()) {
            return; // Achievement already minted for this user
        }

        let metadata = get_nft_metadata(achievement.clone());

        // Mint the NFT
        let nft_address = env.storage().instance().get(&Symbol::short("nft_addr")).unwrap();
        let nft_client = soroban_nft::Client::new(&env, &nft_address);
        nft_client.mint(&to, &metadata.uri);

        // Mark the achievement as minted for the user
        portfolio.minted_achievements.set((to, achievement), true);
        env.storage().instance().set(&(), &portfolio);
    }
}