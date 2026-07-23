
use soroban_sdk::{contracttype, contracterror, Address, Env, Vec, Map};

use crate::rewards::{self, Badge};

const MAX_LEADERBOARD_SIZE: u32 = 100;

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Season {
    pub id: u64,
    pub start_time: u64,
    pub end_time: u64,
    pub ended: bool,
}

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct UserSeasonScore {
    pub volume: i128,
    pub score: i128,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum SeasonError {
    SeasonNotEnded = 1,
    SeasonAlreadyExists = 2,
    NoSeasonActive = 3,
    SeasonStillActive = 4,
    NotAdmin = 5,
}

#[contracttype]
pub enum DataKey {
    Seasons(u64),
    CurrentSeason,
    SeasonScores(u64, Address),
    SeasonLeaderboard(u64),
}

pub fn start_season(env: &Env, end_time: u64) -> Result<(), SeasonError> {
    let current_season_id = env.storage().instance().get(&DataKey::CurrentSeason).unwrap_or(0u64);
    if current_season_id != 0 {
        let current_season: Season = env.storage().instance().get(&DataKey::Seasons(current_season_id)).unwrap();
        if !current_season.ended {
            return Err(SeasonError::SeasonStillActive);
        }
    }

    let new_season_id = current_season_id + 1;
    let new_season = Season {
        id: new_season_id,
        start_time: env.ledger().timestamp(),
        end_time,
        ended: false,
    };

    env.storage().instance().set(&DataKey::Seasons(new_season_id), &new_season);
    env.storage().instance().set(&DataKey::CurrentSeason, &new_season_id);

    Ok(())
}

pub fn end_season(env: &Env) -> Result<(), SeasonError> {
    let current_season_id: u64 = env.storage().instance().get(&DataKey::CurrentSeason).ok_or(SeasonError::NoSeasonActive)?;
    let mut current_season: Season = env.storage().instance().get(&DataKey::Seasons(current_season_id)).unwrap();

    if current_season.ended {
        return Ok(());
    }

    current_season.ended = true;
    env.storage().instance().set(&DataKey::Seasons(current_season_id), &current_season);

    // Snapshot top N traders and award badges
    let leaderboard = get_season_leaderboard(env, current_season_id)?;
    for (i, (user, _score)) in leaderboard.iter().enumerate() {
        if i < 3 { // Award badges to top 3
            rewards::award_badge(env, user, Badge::TopTrader);
        }
    }

    Ok(())
}

pub fn get_season_leaderboard(env: &Env, season_id: u64) -> Result<Vec<(Address, i128)>, SeasonError> {
    let season: Season = env.storage().instance().get(&DataKey::Seasons(season_id)).ok_or(SeasonError::SeasonNotEnded)?;
    if !season.ended {
        return Err(SeasonError::SeasonNotEnded);
    }
    let leaderboard = env.storage().instance().get(&DataKey::SeasonLeaderboard(season_id)).unwrap_or_else(|| Vec::new(env));
    Ok(leaderboard)
}

pub fn get_current_season(env: &Env) -> Result<Season, SeasonError> {
    let current_season_id: u64 = env.storage().instance().get(&DataKey::CurrentSeason).ok_or(SeasonError::NoSeasonActive)?;
    env.storage().instance().get(&DataKey::Seasons(current_season_id)).ok_or(SeasonError::NoSeasonActive)
}

pub fn update_user_score(env: &Env, user: &Address, trade_volume: i128) -> Result<(), SeasonError> {
    let current_season_id: u64 = env.storage().instance().get(&DataKey::CurrentSeason).ok_or(SeasonError::NoSeasonActive)?;
    let mut user_score: UserSeasonScore = env.storage().instance().get(&DataKey::SeasonScores(current_season_id, user.clone())).unwrap_or(UserSeasonScore { volume: 0, score: 0 });

    user_score.volume += trade_volume;
    user_score.score += trade_volume; // For now, score is 1:1 with volume

    env.storage().instance().set(&DataKey::SeasonScores(current_season_id, user.clone()), &user_score);

    update_leaderboard(env, current_season_id, user.clone(), user_score.score);

    Ok(())
}

fn update_leaderboard(env: &Env, season_id: u64, user: Address, score: i128) {
    let mut leaderboard: Vec<(Address, i128)> = env.storage().instance().get(&DataKey::SeasonLeaderboard(season_id)).unwrap_or_else(|| Vec::new(env));

    let mut user_index: Option<u32> = None;
    for (i, (leaderboard_user, _)) in leaderboard.iter().enumerate() {
        if leaderboard_user == user {
            user_index = Some(i as u32);
            break;
        }
    }

    if let Some(index) = user_index {
        leaderboard.remove(index);
    }

    leaderboard.push_back((user, score));
    // The `sort_by_key` is not available in soroban, so we need to do it manually
    // The following is a simple insertion sort, which is not efficient for large leaderboards
    for i in (0..leaderboard.len().saturating_sub(1)).rev() {
        if leaderboard.get(i).unwrap().1 < leaderboard.get(i + 1).unwrap().1 {
            let temp = leaderboard.get(i + 1).unwrap();
            leaderboard.set(i + 1, leaderboard.get(i).unwrap());
            leaderboard.set(i, temp);
        } else {
            break;
        }
    }


    if leaderboard.len() > MAX_LEADERBOARD_SIZE {
        leaderboard.pop_back();
    }

    env.storage().instance().set(&DataKey::SeasonLeaderboard(season_id), &leaderboard);
}