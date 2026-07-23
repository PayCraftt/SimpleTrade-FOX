use soroban_sdk::{contracttype, Address, Env, Symbol};

#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum BridgeStatus {
    Pending,
    Locked,
    Released,
    Failed,
}

#[contracttype]
#[derive(Clone, Debug)]
pub struct BridgeRequest {
    pub id: u64,
    pub sender: Address,
    pub recipient: Address,
    pub amount: i128,
    pub source_chain: Symbol,
    pub dest_chain: Symbol,
    pub status: BridgeStatus,
}

const BRIDGE_COUNTER_KEY: &str = "bridge_counter";
const BRIDGE_REQUEST_PREFIX: &str = "bridge_req";

pub fn initiate_bridge(
    env: &Env,
    sender: Address,
    recipient: Address,
    amount: i128,
    source_chain: Symbol,
    dest_chain: Symbol,
) -> u64 {
    sender.require_auth();

    assert!(amount > 0, "Amount must be positive");

    let id: u64 = env
        .storage()
        .instance()
        .get(&soroban_sdk::symbol_short!("brcnt"))
        .unwrap_or(0u64)
        + 1;

    env.storage()
        .instance()
        .set(&soroban_sdk::symbol_short!("brcnt"), &id);

    let request = BridgeRequest {
        id,
        sender,
        recipient,
        amount,
        source_chain,
        dest_chain,
        status: BridgeStatus::Locked,
    };

    let key = (soroban_sdk::symbol_short!("brreq"), id);
    env.storage().persistent().set(&key, &request);

    id
}

pub fn confirm_bridge(env: &Env, oracle: Address, request_id: u64) {
    oracle.require_auth();

    let key = (soroban_sdk::symbol_short!("brreq"), request_id);
    let mut request: BridgeRequest = env
        .storage()
        .persistent()
        .get(&key)
        .expect("Bridge request not found");

    assert!(
        request.status == BridgeStatus::Locked,
        "Request is not in Locked state"
    );

    request.status = BridgeStatus::Released;
    env.storage().persistent().set(&key, &request);
}

pub fn fail_bridge(env: &Env, oracle: Address, request_id: u64) {
    oracle.require_auth();

    let key = (soroban_sdk::symbol_short!("brreq"), request_id);
    let mut request: BridgeRequest = env
        .storage()
        .persistent()
        .get(&key)
        .expect("Bridge request not found");

    assert!(
        request.status == BridgeStatus::Locked,
        "Request is not in Locked state"
    );

    request.status = BridgeStatus::Failed;
    env.storage().persistent().set(&key, &request);
}

pub fn get_bridge_request(env: &Env, request_id: u64) -> BridgeRequest {
    let key = (soroban_sdk::symbol_short!("brreq"), request_id);
    env.storage()
        .persistent()
        .get(&key)
        .expect("Bridge request not found")
}
