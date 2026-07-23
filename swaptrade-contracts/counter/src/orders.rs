use soroban_sdk::{contracttype, symbol_short, Address, Env, Map, Symbol, Vec};

use crate::errors::ContractError;

/// Order types supported by the system
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum OrderType {
    Market,    // Execute immediately at best available price
    Limit,     // Execute only at specified price or better
    StopLoss,  // Execute when price reaches trigger (becomes market order)
    StopLimit, // Execute when price reaches trigger (becomes limit order)
}

/// Order status
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub enum OrderStatus {
    Pending,         // Order is active and waiting to be filled
    Filled,          // Order has been completely filled
    Cancelled,       // Order was cancelled by user
    Expired,         // Order expired without being filled
    PartiallyFilled, // Order is partially executed
}

/// A trade order in the system
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct Order {
    pub order_id: u64,
    pub owner: Address,
    pub order_type: OrderType,
    pub token_in: Symbol,
    pub token_out: Symbol,
    pub amount_in: i128,
    pub amount_filled: i128,
    pub limit_price: Option<u128>, // For limit orders: minimum acceptable price
    pub trigger_price: Option<u128>, // For stop orders: price that triggers execution
    pub status: OrderStatus,
    pub created_at: u64,
    pub expires_at: Option<u64>, // None means no expiry
    pub filled_at: Option<u64>,
}

/// Order book for a token pair
#[contracttype]
#[derive(Clone, Debug, PartialEq)]
pub struct OrderBook {
    pub token_pair: (Symbol, Symbol),
    pub buy_orders: Vec<u64>,  // Order IDs for buy orders
    pub sell_orders: Vec<u64>, // Order IDs for sell orders
}

/// Order manager - handles order lifecycle
pub struct OrderManager;

impl OrderManager {
    /// Place a limit order
    /// Will execute when market price reaches limit_price or better
    pub fn place_limit_order(
        env: &Env,
        owner: Address,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
        limit_price: u128,
        expires_at: Option<u64>,
    ) -> Result<u64, ContractError> {
        owner.require_auth();

        if amount_in <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        if limit_price == 0 {
            return Err(ContractError::InvalidPrice);
        }

        Self::create_order(
            env,
            owner,
            OrderType::Limit,
            token_in,
            token_out,
            amount_in,
            Some(limit_price),
            None,
            expires_at,
        )
    }

    /// Place a stop-loss order
    /// Will execute as market order when price reaches trigger_price
    pub fn place_stop_loss(
        env: &Env,
        owner: Address,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
        trigger_price: u128,
        expires_at: Option<u64>,
    ) -> Result<u64, ContractError> {
        owner.require_auth();

        if amount_in <= 0 {
            return Err(ContractError::InvalidAmount);
        }

        if trigger_price == 0 {
            return Err(ContractError::InvalidPrice);
        }

        Self::create_order(
            env,
            owner,
            OrderType::StopLoss,
            token_in,
            token_out,
            amount_in,
            None,
            Some(trigger_price),
            expires_at,
        )
    }

    /// Cancel an order
    pub fn cancel_order(env: &Env, order_id: u64, owner: Address) -> Result<(), ContractError> {
        owner.require_auth();

        let mut order = Self::get_order(env, order_id)?;

        if order.owner != owner {
            return Err(ContractError::NotAdmin); // No specific "NotOrderOwner" error
        }

        if order.status != OrderStatus::Pending && order.status != OrderStatus::PartiallyFilled {
            return Err(ContractError::InvalidAmount); // Order cannot be cancelled
        }

        order.status = OrderStatus::Cancelled;
        Self::save_order(env, &order);

        // Emit cancellation event
        env.events().publish(
            (symbol_short!("ordcan"), order_id),
            (owner, order.token_in, order.token_out, order.amount_in),
        );

        Ok(())
    }

    /// Check and execute pending orders that can be filled
    /// Called during trading operations to match orders
    pub fn match_pending_orders(
        env: &Env,
        token_in: Symbol,
        token_out: Symbol,
        current_price: u128,
    ) -> Result<Vec<u64>, ContractError> {
        let mut executed_orders = Vec::new(env);
        let pair_key = Self::order_book_key(&(token_in.clone(), token_out.clone()));

        // Get order book for this pair
        let order_book: Option<OrderBook> = env.storage().instance().get(&pair_key);

        if order_book.is_none() {
            return Ok(executed_orders);
        }

        let book = order_book.unwrap();
        let current_time = env.ledger().timestamp();

        // Check buy orders: execute if current_price <= limit_price
        for i in 0..book.buy_orders.len() {
            if let Some(order_id) = book.buy_orders.get(i) {
                if let Ok(mut order) = Self::get_order(env, order_id) {
                    // Check expiry
                    if let Some(expires) = order.expires_at {
                        if current_time > expires {
                            order.status = OrderStatus::Expired;
                            Self::save_order(env, &order);
                            continue;
                        }
                    }

                    // Check if order can be executed
                    if order.status == OrderStatus::Pending
                        || order.status == OrderStatus::PartiallyFilled
                    {
                        let should_execute = match order.order_type {
                            OrderType::Limit => {
                                // Execute if current price is at or below limit
                                if let Some(limit) = order.limit_price {
                                    current_price <= limit
                                } else {
                                    false
                                }
                            }
                            OrderType::StopLoss => {
                                // Execute if current price is at or below trigger
                                if let Some(trigger) = order.trigger_price {
                                    current_price <= trigger
                                } else {
                                    false
                                }
                            }
                            _ => false,
                        };

                        if should_execute {
                            // Mark as filled (actual execution happens in trading module)
                            order.status = OrderStatus::Filled;
                            order.filled_at = Some(current_time);
                            Self::save_order(env, &order);
                            executed_orders.push_back(order_id);

                            // Emit execution event
                            env.events().publish(
                                (symbol_short!("ofill"), order_id),
                                (
                                    order.owner,
                                    token_in.clone(),
                                    token_out.clone(),
                                    current_price,
                                ),
                            );
                        }
                    }
                }
            }
        }

        Ok(executed_orders)
    }

    /// Get order details
    pub fn get_order(env: &Env, order_id: u64) -> Result<Order, ContractError> {
        let order_key = Self::order_key(order_id);
        env.storage()
            .instance()
            .get(&order_key)
            .ok_or(ContractError::InvalidAmount) // Order not found
    }

    /// Get user's active orders
    pub fn get_user_orders(env: &Env, user: Address) -> Vec<Order> {
        let mut orders = Vec::new(env);
        let user_order_ids: Option<Vec<u64>> =
            env.storage().instance().get(&Self::user_orders_key(&user));

        if let Some(order_ids) = user_order_ids {
            for i in 0..order_ids.len() {
                if let Some(order_id) = order_ids.get(i) {
                    if let Ok(order) = Self::get_order(env, order_id) {
                        if order.status == OrderStatus::Pending
                            || order.status == OrderStatus::PartiallyFilled
                        {
                            orders.push_back(order);
                        }
                    }
                }
            }
        }

        orders
    }

    /// Create a new order
    fn create_order(
        env: &Env,
        owner: Address,
        order_type: OrderType,
        token_in: Symbol,
        token_out: Symbol,
        amount_in: i128,
        limit_price: Option<u128>,
        trigger_price: Option<u128>,
        expires_at: Option<u64>,
    ) -> Result<u64, ContractError> {
        // Generate order ID
        let next_id: u64 = env
            .storage()
            .instance()
            .get(&symbol_short!("next_oid"))
            .unwrap_or(1);

        let order = Order {
            order_id: next_id,
            owner: owner.clone(),
            order_type: order_type.clone(),
            token_in: token_in.clone(),
            token_out: token_out.clone(),
            amount_in,
            amount_filled: 0,
            limit_price,
            trigger_price,
            status: OrderStatus::Pending,
            created_at: env.ledger().timestamp(),
            expires_at,
            filled_at: None,
        };

        // Save order
        Self::save_order(env, &order);

        // Add to user's order list
        let mut user_orders: Vec<u64> = env
            .storage()
            .instance()
            .get(&Self::user_orders_key(&owner))
            .unwrap_or_else(|| Vec::new(env));
        user_orders.push_back(next_id);
        env.storage()
            .instance()
            .set(&Self::user_orders_key(&owner), &user_orders);

        // Add to order book
        Self::add_to_order_book(env, token_in.clone(), token_out.clone(), next_id);

        // Increment next order ID
        env.storage()
            .instance()
            .set(&symbol_short!("next_oid"), &(next_id + 1));

        // Emit order placement event
        env.events().publish(
            (symbol_short!("order_new"), next_id),
            (
                owner,
                order_type,
                token_in,
                token_out,
                amount_in,
                limit_price,
                trigger_price,
            ),
        );

        Ok(next_id)
    }

    /// Save order to storage
    fn save_order(env: &Env, order: &Order) {
        let order_key = Self::order_key(order.order_id);
        env.storage().instance().set(&order_key, order);
    }

    /// Add order to order book
    fn add_to_order_book(env: &Env, token_in: Symbol, token_out: Symbol, order_id: u64) {
        let pair = (token_in.clone(), token_out.clone());
        let pair_key = Self::order_book_key(&pair);

        let mut book: OrderBook = env
            .storage()
            .instance()
            .get(&pair_key)
            .unwrap_or(OrderBook {
                token_pair: pair.clone(),
                buy_orders: Vec::new(env),
                sell_orders: Vec::new(env),
            });

        // Determine if buy or sell order (simplified: based on token ordering)
        // In reality, this would depend on whether user is buying or selling
        book.buy_orders.push_back(order_id);

        env.storage().instance().set(&pair_key, &book);
    }

    fn order_key(order_id: u64) -> (Symbol, u64) {
        (symbol_short!("order"), order_id)
    }

    fn user_orders_key(user: &Address) -> (Symbol, Address) {
        (symbol_short!("uorders"), user.clone())
    }

    fn order_book_key(pair: &(Symbol, Symbol)) -> (Symbol, Symbol, Symbol) {
        (symbol_short!("obook"), pair.0.clone(), pair.1.clone())
    }
}
