use soroban_sdk::contracterror;

/// Unified error catalog for SwapTrade contracts.
///
/// Code ranges:
///   1–9      Admin / access control
///   10–19    Trading / contract state
///   100–109  Validation (amounts, tokens, pairs)
///   200–209  Oracle / invariants
///   300–309  Rate limiting / slippage
///   400–409  Liquidity pool
///   500–509  KYC
///   600–609  Staking
///   700–709  Emergency / circuit-breaker
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum SwapTradeError {
    // ── Admin / access control ──────────────────────────────────────────────
    NotAdmin = 1,
    NotAuthorized = 2,
    InvalidAddress = 3,
    InvalidMultiSigConfig = 4,
    MultiSigNotConfigured = 5,

    // ── Governance ──────────────────────────────────────────────────────────
    ProposalNotFound = 90,
    ProposalAlreadyExecuted = 91,
    AlreadyApproved = 92,
    InsufficientApprovals = 93,

    // ── Trading / contract state ────────────────────────────────────────────
    TradingPaused = 10,
    UserFrozen = 11,
    CircuitBreakerTripped = 12,
    InvalidPrivateTransaction = 13,

    // ── Validation ──────────────────────────────────────────────────────────
    InvalidAmount = 100,
    AmountOverflow = 101,
    InvalidTokenSymbol = 102,
    InvalidSwapPair = 103,
    InsufficientBalance = 104,
    ZeroAmountSwap = 105,

    // ── Oracle / invariants ─────────────────────────────────────────────────
    InvariantViolation = 200,
    StalePrice = 201,
    InvalidPrice = 202,
    PriceNotSet = 203,
    OracleNotConfigured = 204,
    OracleNotActive = 205,
    CircuitBreakerActive = 206,
    CircuitBreakerTriggered = 207,
    InvalidConfig = 208,

    // ── Rate limiting / slippage ────────────────────────────────────────────
    RateLimitExceeded = 300,
    SlippageExceeded = 301,
    Expired = 302,

    // ── Liquidity pool ──────────────────────────────────────────────────────
    LPPositionNotFound = 400,
    InsufficientLPTokens = 401,

    // ── KYC ─────────────────────────────────────────────────────────────────
    KYCVerificationRequired = 500,
    NotKYCOperator = 501,
    InvalidKYCStateTransition = 502,
    KYCTerminalStateImmutable = 503,
    SelfVerificationNotAllowed = 504,
    KYCOverrideNotFound = 505,
    KYCTimelockNotElapsed = 506,
    KYCOverrideAlreadyExecuted = 507,
    InvalidTimelockDuration = 508,
    KYCRequestExpired = 509,
    InvalidExpiryDuration = 510,
    /// KYC input data exceeds the allowed size limit (#160).
    KYCDataTooLarge = 511,
    /// Maximum number of KYC operators already registered (#160).
    KYCOperatorLimitReached = 512,

    // ── Staking ─────────────────────────────────────────────────────────────
    InvalidStakeDuration = 600,
    StakeNotFound = 601,
    StakeNotActive = 602,
    StakeLocked = 603,
    NoClaimableBonuses = 604,
    DistributionTooEarly = 605,
    // Farming errors
    FarmingPoolNotFound = 606,
    InsufficientStakedLP = 607,

    // ── Emergency / circuit-breaker ─────────────────────────────────────────
    NotEmergencyAdmin = 700,

    // ── Referral system ─────────────────────────────────────────────────────
    SelfReferral = 800,
    AlreadyReferred = 801,
    CircularReferral = 802,

    // ── Faucet ─────────────────────────────────────────────────────────────
    FaucetRateLimited = 900,
    FaucetNotConfigured = 901,
}

/// Alias kept for modules that still import `ContractError` by name.
pub type ContractError = SwapTradeError;