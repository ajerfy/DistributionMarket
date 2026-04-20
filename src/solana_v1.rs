use crate::fixed_point::Fixed;
use crate::normal_market::FixedNormalMarket;
use crate::normal_math::FixedNormalDistribution;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarketStatus {
    Uninitialized,
    Active,
    Resolved,
    Settled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalPositionSide {
    Trade,
    Liquidity,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiquidityAction {
    Add,
    Remove,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OracleConfigV1 {
    pub oracle_program: [u8; 32],
    pub oracle_feed: [u8; 32],
    pub authority: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolanaMarketAccountV1 {
    pub version: u8,
    pub bump: u8,
    pub status: MarketStatus,
    pub collateral_mint: [u8; 32],
    pub collateral_vault: [u8; 32],
    pub lp_mint: [u8; 32],
    pub market_authority: [u8; 32],
    pub oracle_config: OracleConfigV1,
    pub b: Fixed,
    pub k: Fixed,
    pub current_distribution: FixedNormalDistribution,
    pub current_lambda: Fixed,
    pub total_lp_shares: Fixed,
    pub total_trades: u64,
    pub resolved_outcome: Option<Fixed>,
    pub created_slot: u64,
    pub resolved_slot: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolanaNormalPositionAccountV1 {
    pub version: u8,
    pub bump: u8,
    pub market: [u8; 32],
    pub owner: [u8; 32],
    pub side: NormalPositionSide,
    pub id: u64,
    pub old_distribution: FixedNormalDistribution,
    pub new_distribution: FixedNormalDistribution,
    pub collateral_posted: Fixed,
    pub lp_shares: Fixed,
    pub settled: bool,
    pub payout_claimed: Fixed,
    pub created_slot: u64,
    pub settled_slot: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuoteEnvelopeV1 {
    pub market: [u8; 32],
    pub expected_market_version: u64,
    pub new_distribution: FixedNormalDistribution,
    pub collateral_required: Fixed,
    pub max_slippage_collateral: Fixed,
    pub search_lower_bound: Fixed,
    pub search_upper_bound: Fixed,
    pub coarse_samples: u32,
    pub refine_samples: u32,
    pub quote_slot: u64,
    pub quote_expiry_slot: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeArgsV1 {
    pub quote: QuoteEnvelopeV1,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolveMarketArgsV1 {
    pub outcome: Fixed,
    pub oracle_observation_slot: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettlePositionArgsV1 {
    pub position_id: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettleLpArgsV1 {
    pub owner: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SolanaInstructionV1 {
    InitializeMarket {
        initial_b: Fixed,
        initial_k: Fixed,
        initial_distribution: FixedNormalDistribution,
        oracle_config: OracleConfigV1,
    },
    Trade(TradeArgsV1),
    ManageLiquidity {
        action: LiquidityAction,
        owner: [u8; 32],
        amount_or_shares: Fixed,
    },
    ResolveMarket(ResolveMarketArgsV1),
    SettlePosition(SettlePositionArgsV1),
    SettleLp(SettleLpArgsV1),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolanaPositionSettlementV1 {
    pub position_id: u64,
    pub payout: Fixed,
    pub collateral_returned: Fixed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SolanaOperationMappingV1 {
    pub fixed_normal_market_method: &'static str,
    pub solana_instruction: &'static str,
    pub account_effects: &'static str,
    pub verification_notes: &'static str,
}

pub fn current_normal_market_to_account(
    market: &FixedNormalMarket,
    collateral_mint: [u8; 32],
    collateral_vault: [u8; 32],
    lp_mint: [u8; 32],
    market_authority: [u8; 32],
    oracle_config: OracleConfigV1,
) -> SolanaMarketAccountV1 {
    SolanaMarketAccountV1 {
        version: 1,
        bump: 0,
        status: MarketStatus::Active,
        collateral_mint,
        collateral_vault,
        lp_mint,
        market_authority,
        oracle_config,
        b: market.b,
        k: market.k,
        current_distribution: market.current_distribution,
        current_lambda: market.current_lambda,
        total_lp_shares: market.total_lp_shares,
        total_trades: market.trades.len() as u64,
        resolved_outcome: None,
        created_slot: 0,
        resolved_slot: None,
    }
}

pub fn normal_v1_operation_mapping() -> Vec<SolanaOperationMappingV1> {
    vec![
        SolanaOperationMappingV1 {
            fixed_normal_market_method: "FixedNormalMarket::new",
            solana_instruction: "InitializeMarket",
            account_effects: "Creates Market PDA, collateral vault, LP mint, and mints genesis LP shares to the initializer.",
            verification_notes: "Program checks positive b/k, validates sigma floor, computes lambda, and stores the active Normal distribution in fixed point.",
        },
        SolanaOperationMappingV1 {
            fixed_normal_market_method: "FixedNormalMarket::quote_trade + verify_trade_quote + trade_with_quote",
            solana_instruction: "Trade",
            account_effects: "Transfers collateral into the vault, increments market trade counter, and creates a Position PDA for the trader.",
            verification_notes: "Client supplies a bounded quote envelope; program verifies market version, quote expiry, sigma floor, search bounds, sample counts, and that collateral covers the deterministic conservative verifier result.",
        },
        SolanaOperationMappingV1 {
            fixed_normal_market_method: "FixedNormalMarket::add_liquidity",
            solana_instruction: "ManageLiquidity(Add)",
            account_effects: "Transfers backing into the vault, increases market b/k/lambda state, and mints fungible LP shares to the provider.",
            verification_notes: "Program verifies proportional scaling from current state and records LP share growth without creating a separate trade position.",
        },
        SolanaOperationMappingV1 {
            fixed_normal_market_method: "FixedNormalMarket::remove_liquidity",
            solana_instruction: "ManageLiquidity(Remove)",
            account_effects: "Burns LP shares, transfers backing out of the vault, and scales b/k/lambda downward.",
            verification_notes: "Program checks the signer's LP balance and rejects withdrawals that would violate the market's active-state invariants.",
        },
        SolanaOperationMappingV1 {
            fixed_normal_market_method: "FixedNormalMarket::resolve",
            solana_instruction: "ResolveMarket",
            account_effects: "Writes the final outcome and resolved slot onto the Market account, freezing further trades and liquidity changes.",
            verification_notes: "Program validates oracle authority/feed semantics and allows resolution only once.",
        },
        SolanaOperationMappingV1 {
            fixed_normal_market_method: "FixedNormalMarket::resolve (trade branch)",
            solana_instruction: "SettlePosition",
            account_effects: "Transfers payout from the vault to the position owner and marks the Position PDA as settled.",
            verification_notes: "Program recomputes deterministic payout from stored old/new distributions and the resolved outcome; settlement is idempotent per position.",
        },
        SolanaOperationMappingV1 {
            fixed_normal_market_method: "FixedNormalMarket::resolve (LP branch)",
            solana_instruction: "SettleLp",
            account_effects: "Burns LP shares and transfers the owner's pro rata remaining vault balance after trader settlements.",
            verification_notes: "Program computes claim from remaining funds and current LP supply, then prevents double-claiming by burning or checkpointing LP shares.",
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{LiquidityAction, MarketStatus, OracleConfigV1, SolanaInstructionV1, normal_v1_operation_mapping};
    use crate::fixed_point::Fixed;
    use crate::normal_math::FixedNormalDistribution;

    #[test]
    fn normal_v1_mapping_covers_core_market_operations() {
        let mapping = normal_v1_operation_mapping();
        assert_eq!(mapping.len(), 7);
        assert!(mapping.iter().any(|entry| entry.solana_instruction == "InitializeMarket"));
        assert!(mapping.iter().any(|entry| entry.solana_instruction == "Trade"));
        assert!(mapping.iter().any(|entry| entry.solana_instruction == "ManageLiquidity(Add)"));
        assert!(mapping.iter().any(|entry| entry.solana_instruction == "ManageLiquidity(Remove)"));
        assert!(mapping.iter().any(|entry| entry.solana_instruction == "ResolveMarket"));
        assert!(mapping.iter().any(|entry| entry.solana_instruction == "SettlePosition"));
        assert!(mapping.iter().any(|entry| entry.solana_instruction == "SettleLp"));
    }

    #[test]
    fn initialize_market_instruction_holds_normal_v1_payload() {
        let instruction = SolanaInstructionV1::InitializeMarket {
            initial_b: Fixed::from_f64(50.0).unwrap(),
            initial_k: Fixed::from_f64(21.05026039569057).unwrap(),
            initial_distribution: FixedNormalDistribution::new(
                Fixed::from_f64(95.0).unwrap(),
                Fixed::from_f64(10.0).unwrap(),
            )
            .unwrap(),
            oracle_config: OracleConfigV1 {
                oracle_program: [1_u8; 32],
                oracle_feed: [2_u8; 32],
                authority: [3_u8; 32],
            },
        };

        match instruction {
            SolanaInstructionV1::InitializeMarket { initial_b, initial_k, .. } => {
                assert_eq!(initial_b, Fixed::from_f64(50.0).unwrap());
                assert_eq!(initial_k, Fixed::from_f64(21.05026039569057).unwrap());
            }
            _ => panic!("expected initialize_market variant"),
        }
    }

    #[test]
    fn enums_are_stable_for_v1_layout_discussion() {
        let add = LiquidityAction::Add;
        let remove = LiquidityAction::Remove;
        assert_ne!(format!("{add:?}"), format!("{remove:?}"));
        let status = MarketStatus::Active;
        assert_eq!(format!("{status:?}"), "Active");
    }
}
