#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

// Make the WASM binary available.
#[cfg(feature = "std")]
include!(concat!(env!("OUT_DIR"), "/wasm_binary.rs"));

mod weights;
pub mod xcm_config;

extern crate alloc;
use alloc::borrow::Cow;
use alloc::{vec, vec::Vec};
use cumulus_pallet_parachain_system::RelayNumberMonotonicallyIncreases;
use currency::CENTS;
use pallet_aura::Authorities;
use sp_api::impl_runtime_apis;
use sp_core::{crypto::KeyTypeId, Get, OpaqueMetadata, H256};
use sp_runtime::{generic, impl_opaque_keys, traits::{BlakeTwo256, Block as BlockT, IdentifyAccount, Verify}, transaction_validity::{TransactionSource, TransactionValidity}, ApplyExtrinsicResult, MultiSignature, Perquintill};
use static_assertions::const_assert;

pub use types::currency;

#[cfg(feature = "std")]
use sp_version::NativeVersion;
use sp_version::RuntimeVersion;

use codec::MaxEncodedLen;
use cumulus_primitives_core::AggregateMessageOrigin;
use frame_support::{
    construct_runtime, derive_impl,
    dispatch::{DispatchClass, DispatchResult},
    genesis_builder_helper::{build_state, get_preset},
    parameter_types,
    traits::{
        fungible::HoldConsideration, ConstU32, ConstU64, LinearStoragePrice,
    },
    weights::{
        constants::WEIGHT_REF_TIME_PER_SECOND, ConstantMultiplier, Weight,
    },
    Blake2_128Concat, Identity, PalletId, StorageHasher,
};
use frame_support::traits::Footprint;
use frame_system::{
    limits::{BlockLength, BlockWeights},
    EnsureRoot,
};
use hp_dispatch::{Destination, DispatchAggregation};

pub use sp_consensus_aura::sr25519::AuthorityId as AuraId;
pub use sp_runtime::{MultiAddress, Perbill, Permill};

pub mod types;

#[cfg(any(feature = "std", test))]
pub use sp_runtime::BuildStorage;

// Polkadot imports
use polkadot_runtime_common::{BlockHashCount, SlowAdjustingFeeUpdate};
use sp_runtime::traits::Convert;
use weights::{BlockExecutionWeight, ExtrinsicBaseWeight, RocksDbWeight};


pub(crate) mod weight_aliases {
    pub mod pallet_plonky2_verifier_verify_proof {
        pub use pallet_plonky2_verifier::WeightInfoVerifyProof as WeightInfo;
    }

    pub mod pallet_risc0_verifier_verify_proof {
        pub use pallet_risc0_verifier::WeightInfoVerifyProof as WeightInfo;
    }

    pub mod frame_system_extensions {
        pub use frame_system::ExtensionsWeightInfo as WeightInfo;
    }
}

// XCM Imports
// use xcm::latest::prelude::BodyId;

/// Alias to 512-bit hash when used in the context of a transaction signature on the chain.
pub type Signature = MultiSignature;

/// Some way of identifying an account on the chain. We intentionally make it equivalent
/// to the public key of our transaction signing scheme.
pub type AccountId = <<Signature as Verify>::Signer as IdentifyAccount>::AccountId;

/// Balance of an account.
pub type Balance = u128;

/// Index of a transaction in the chain.
pub type Nonce = u32;

/// A hash of some data used by the chain.
pub type Hash = sp_core::H256;

/// An index to a block.
pub type BlockNumber = u32;

/// The address format for describing accounts.
pub type Address = MultiAddress<AccountId, ()>;

/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;

/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;

/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;

/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;

/// The SignedExtension to the basic transaction logic.
pub type TxExtension = (
    frame_system::CheckNonZeroSender<Runtime>,
    frame_system::CheckSpecVersion<Runtime>,
    frame_system::CheckTxVersion<Runtime>,
    frame_system::CheckGenesis<Runtime>,
    frame_system::CheckEra<Runtime>,
    frame_system::CheckNonce<Runtime>,
    frame_system::CheckWeight<Runtime>,
    pallet_transaction_payment::ChargeTransactionPayment<Runtime>,
);

/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic =
    generic::UncheckedExtrinsic<Address, RuntimeCall, Signature, TxExtension>;

/// Migrations to apply on runtime upgrade.
pub type Migrations = (
    pallet_collator_selection::migration::v2::MigrationToV2<Runtime>,
    // permanent
    // pallet_xcm::migration::MigrateToLatestXcmVersion<Runtime>,
);

/// Executive: handles dispatch to the various modules.
pub type Executive = frame_executive::Executive<
    Runtime,
    Block,
    frame_system::ChainContext<Runtime>,
    Runtime,
    AllPalletsWithSystem,
    Migrations
>;

/// Opaque types. These are used by the CLI to instantiate machinery that don't need to know
/// the specifics of the runtime. They can then be made to be agnostic over specific formats
/// of data like extrinsics, allowing for them to continue syncing the network through upgrades
/// to even the core data structures.
pub mod opaque {
    use super::*;
    use sp_runtime::{
        generic,
        traits::{BlakeTwo256, Hash as HashT},
    };

    pub use sp_runtime::OpaqueExtrinsic as UncheckedExtrinsic;
    /// Opaque block header type.
    pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
    /// Opaque block type.
    pub type Block = generic::Block<Header, UncheckedExtrinsic>;
    /// Opaque block identifier type.
    pub type BlockId = generic::BlockId<Block>;
    /// Opaque block hash type.
    pub type Hash = <BlakeTwo256 as HashT>::Output;
}

impl_opaque_keys! {
    pub struct SessionKeys {
        pub aura: Aura,
    }
}

#[sp_version::runtime_version]
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: Cow::Borrowed("parazkv"),
    impl_name: Cow::Borrowed("parazkv"),
    authoring_version: 1,
    spec_version: 1_000,
    impl_version: 0,
    apis: RUNTIME_API_VERSIONS,
    transaction_version: 1,
    system_version: 1,
};

/// This determines the average expected block time that we are targeting.
/// Blocks will be produced at a minimum duration defined by `SLOT_DURATION`.
/// `SLOT_DURATION` is picked up by `pallet_timestamp` which is in turn picked
/// up by `pallet_aura` to implement `fn slot_duration()`.
///
/// Change this to adjust the block time.
pub const MILLISECS_PER_BLOCK: u64 = 6000;

// NOTE: Currently it is not possible to change the slot duration after the chain has started.
//       Attempting to do so will brick block production.
pub const SLOT_DURATION: u64 = MILLISECS_PER_BLOCK;

// Time is measured by number of blocks.
pub const MINUTES: BlockNumber = 60_000 / (MILLISECS_PER_BLOCK as BlockNumber);
pub const HOURS: BlockNumber = MINUTES * 60;
pub const DAYS: BlockNumber = HOURS * 24;

// Unit = the base number of indivisible units for balances
pub const UNIT: Balance = 1_000_000_000_000_000_000;
pub const CENTIUNIT: Balance = UNIT / 100;
pub const MILLIUNIT: Balance = UNIT / 1_000;
pub const MICROUNIT: Balance = MILLIUNIT / 1_000;

/// The existential deposit. Set to 1/10 of the Connected Relay Chain.
pub const EXISTENTIAL_DEPOSIT: Balance = MILLIUNIT;

/// We assume that ~5% of the block weight is consumed by `on_initialize` handlers. This is
/// used to limit the maximal weight of a single extrinsic.
const AVERAGE_ON_INITIALIZE_RATIO: Perbill = Perbill::from_percent(5);

/// We allow `Normal` extrinsics to fill up the block up to 75%, the rest can be used by
/// `Operational` extrinsics.
const NORMAL_DISPATCH_RATIO: Perbill = Perbill::from_percent(75);

/// We allow for 0.5 of a second of compute with a 12 second average block time.
const MAXIMUM_BLOCK_WEIGHT: Weight = Weight::from_parts(
    WEIGHT_REF_TIME_PER_SECOND.saturating_div(2),
    cumulus_primitives_core::relay_chain::MAX_POV_SIZE as u64,
);

/// Maximum number of blocks simultaneously accepted by the Runtime, not yet included
/// into the relay chain.
const UNINCLUDED_SEGMENT_CAPACITY: u32 = 3;
/// How many parachain blocks are processed by the relay chain per parent. Limits the
/// number of blocks authored per slot.
const BLOCK_PROCESSING_VELOCITY: u32 = 1;
/// Relay chain slot duration, in milliseconds.
const RELAY_CHAIN_SLOT_DURATION_MILLIS: u32 = 6000;

/// The version information used to identify this runtime when compiled natively.
#[cfg(feature = "std")]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

parameter_types! {
    pub const Version: RuntimeVersion = VERSION;

    // This part is copied from Substrate's `bin/node/runtime/src/lib.rs`.
    //  The `RuntimeBlockLength` and `RuntimeBlockWeights` exist here because the
    // `DeletionWeightLimit` and `DeletionQueueDepth` depend on those to parameterize
    // the lazy contract deletion.
    pub RuntimeBlockLength: BlockLength =
        BlockLength::max_with_normal_ratio(5 * 1024 * 1024, NORMAL_DISPATCH_RATIO);
    pub RuntimeBlockWeights: BlockWeights = BlockWeights::builder()
        .base_block(BlockExecutionWeight::get())
        .for_class(DispatchClass::all(), |weights| {
            weights.base_extrinsic = ExtrinsicBaseWeight::get();
        })
        .for_class(DispatchClass::Normal, |weights| {
            weights.max_total = Some(NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT);
        })
        .for_class(DispatchClass::Operational, |weights| {
            weights.max_total = Some(MAXIMUM_BLOCK_WEIGHT);
            // Operational transactions have some extra reserved space, so that they
            // are included even if block reached `MAXIMUM_BLOCK_WEIGHT`.
            weights.reserved = Some(
                MAXIMUM_BLOCK_WEIGHT - NORMAL_DISPATCH_RATIO * MAXIMUM_BLOCK_WEIGHT
            );
        })
        .avg_block_initialization(AVERAGE_ON_INITIALIZE_RATIO)
        .build_or_panic();
    pub const SS58Prefix: u16 = 42;
}

/// The default types are being injected by [`derive_impl`](`frame_support::derive_impl`) from
/// [`ParaChainDefaultConfig`](`struct@frame_system::config_preludes::ParaChainDefaultConfig`),
/// but overridden as needed.
#[derive_impl(frame_system::config_preludes::ParaChainDefaultConfig as frame_system::DefaultConfig)]
impl frame_system::Config for Runtime {
    /// The identifier used to distinguish between accounts.
    type AccountId = AccountId;
    /// The index type for storing how many extrinsics an account has signed.
    type Nonce = Nonce;
    /// The type for hashing blocks and tries.
    type Hash = Hash;
    /// The block type.
    type Block = Block;
    /// Maximum number of block number to block hash mappings to keep (oldest pruned first).
    type BlockHashCount = BlockHashCount;
    /// Runtime version.
    type Version = Version;
    /// The data to be stored in an account.
    type AccountData = pallet_balances::AccountData<Balance>;
    /// The weight of database operations that the runtime can invoke.
    type DbWeight = RocksDbWeight;
    /// Block & extrinsics weights: base values and limits.
    type BlockWeights = RuntimeBlockWeights;
    /// The maximum length of a block (in bytes).
    type BlockLength = RuntimeBlockLength;
    /// This is used as an identifier of the chain. 42 is the generic substrate prefix.
    type SS58Prefix = SS58Prefix;
    /// The action to take on a Runtime Upgrade
    type OnSetCode = cumulus_pallet_parachain_system::ParachainSetCode<Self>;
    type MaxConsumers = ConstU32<16>;
    type SystemWeightInfo = weights::frame_system::ZKVWeight<Runtime>;
    type ExtensionsWeightInfo = weights::frame_system_extensions::ZKVWeight<Runtime>;
}

impl pallet_timestamp::Config for Runtime {
    /// A timestamp: milliseconds since the unix epoch.
    type Moment = u64;
    type OnTimestampSet = Aura;
    type MinimumPeriod = ConstU64<{ SLOT_DURATION / 2 }>;
    type WeightInfo = weights::pallet_timestamp::ZKVWeight<Runtime>;
}

impl pallet_authorship::Config for Runtime {
    type FindAuthor = pallet_session::FindAccountFromAuthorIndex<Self, Aura>;
    type EventHandler = (CollatorSelection,);
}

parameter_types! {
    pub const ExistentialDeposit: Balance = EXISTENTIAL_DEPOSIT;
}

impl pallet_balances::Config for Runtime {
    /// The ubiquitous event type.
    type RuntimeEvent = RuntimeEvent;
    type RuntimeHoldReason = RuntimeHoldReason;
    type RuntimeFreezeReason = RuntimeFreezeReason;
    type WeightInfo = weights::pallet_balances::ZKVWeight<Runtime>;
    /// The type for recording an account's balance.
    type Balance = Balance;
    type DustRemoval = ();
    type ExistentialDeposit = ExistentialDeposit;
    type AccountStore = System;
    type ReserveIdentifier = [u8; 8];
    type FreezeIdentifier = ();
    type MaxLocks = ConstU32<50>;
    type MaxReserves = ConstU32<50>;
    type MaxFreezes = ConstU32<0>;
    type DoneSlashHandler = ();
}

parameter_types! {
    ///
    /// Relay Chain `TransactionByteFee` / 5
    pub const TransactionByteFee: Balance = 1_000_000;
    /// Relay Chain `TransactionPicosecondFee` / 5
    pub const TransactionPicosecondFee: Balance = 1_000_000;
    pub const TargetBlockFullness: Perquintill = Perquintill::from_percent(75);
    pub const OperationalFeeMultiplier: u8 = 5;
}


impl pallet_transaction_payment::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnChargeTransaction = pallet_transaction_payment::FungibleAdapter<Balances, ()>;
    type WeightToFee = ConstantMultiplier<Balance, TransactionPicosecondFee>;
    type LengthToFee = ConstantMultiplier<Balance, TransactionByteFee>;
    type FeeMultiplierUpdate = SlowAdjustingFeeUpdate<Self>;
    type OperationalFeeMultiplier = OperationalFeeMultiplier;
    type WeightInfo = weights::pallet_transaction_payment::ZKVWeight<Runtime>;
}

impl pallet_sudo::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type WeightInfo = weights::pallet_sudo::ZKVWeight<Runtime>;
}

parameter_types! {
    pub const ReservedXcmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
    pub const ReservedDmpWeight: Weight = MAXIMUM_BLOCK_WEIGHT.saturating_div(4);
    pub const RelayOrigin: AggregateMessageOrigin = AggregateMessageOrigin::Parent;
}

type ConsensusHook = cumulus_pallet_aura_ext::FixedVelocityConsensusHook<
    Runtime,
    RELAY_CHAIN_SLOT_DURATION_MILLIS,
    BLOCK_PROCESSING_VELOCITY,
    UNINCLUDED_SEGMENT_CAPACITY,
>;

impl cumulus_pallet_parachain_system::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnSystemEvent = ();
    type SelfParaId = parachain_info::Pallet<Runtime>;
    type OutboundXcmpMessageSource = XcmpQueue;
    type DmpQueue = frame_support::traits::EnqueueWithOrigin<MessageQueue, RelayOrigin>;
    type ReservedDmpWeight = ReservedDmpWeight;
    type XcmpMessageHandler = XcmpQueue;
    type ReservedXcmpWeight = ReservedXcmpWeight;
    type CheckAssociatedRelayNumber = RelayNumberMonotonicallyIncreases;
    type WeightInfo = weights::cumulus_pallet_parachain_system::ZKVEvmWeight<Runtime>;
    type ConsensusHook = ConsensusHook;
    type SelectCore = cumulus_pallet_parachain_system::DefaultCoreSelector<Self>;
}

impl parachain_info::Config for Runtime {}

parameter_types! {
    pub MessageQueueServiceWeight: Weight = Perbill::from_percent(35) * RuntimeBlockWeights::get().max_block;
}

impl cumulus_pallet_aura_ext::Config for Runtime {}

parameter_types! {
    pub const Period: u32 = 6 * HOURS;
    pub const Offset: u32 = 0;
}

impl pallet_session::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ValidatorId = AccountId;
    // we don't have stash and controller, thus we don't need the convert as well.
    type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
    type ShouldEndSession = pallet_session::PeriodicSessions<Period, Offset>;
    type NextSessionRotation = pallet_session::PeriodicSessions<Period, Offset>;
    type SessionManager = CollatorSelection;
    // Essentially just Aura, but let's be pedantic.
    type SessionHandler = <SessionKeys as sp_runtime::traits::OpaqueKeys>::KeyTypeIdProviders;
    type Keys = SessionKeys;
    type WeightInfo = weights::pallet_session::ZKVWeight<Runtime>;
}

parameter_types! {
    pub const PotId: PalletId = PalletId(*b"PotStake");
    pub const SessionLength: BlockNumber = 6 * HOURS;
    pub const MaxCandidates: u32 = 30;
    pub const MaxInvulnerables: u32 = 10;
    pub const MinEligibleCollators: u32 = 1;
    pub const AllowMultipleBlocksPerSlot: bool = true;
    pub const MaxAuthorities: u32 = 100_000;
}

impl pallet_aura::Config for Runtime {
    type AuthorityId = AuraId;
    type MaxAuthorities = MaxAuthorities;
    type DisabledValidators = ();
    type AllowMultipleBlocksPerSlot = AllowMultipleBlocksPerSlot;
    type SlotDuration = ConstU64<SLOT_DURATION>;
}
pub type CollatorSelectionUpdateOrigin = EnsureRoot<AccountId>;

impl pallet_collator_selection::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type UpdateOrigin = CollatorSelectionUpdateOrigin;
    type PotId = PotId;
    type MaxCandidates = MaxCandidates;
    type MinEligibleCollators = MinEligibleCollators;
    type MaxInvulnerables = MaxInvulnerables;
    // should be a multiple of session or things will get inconsistent
    type KickThreshold = Period;
    type ValidatorId = <Self as frame_system::Config>::AccountId;
    type ValidatorIdOf = pallet_collator_selection::IdentityCollator;
    type ValidatorRegistration = Session;
    type WeightInfo = weights::pallet_collator_selection::ZKVEvmWeight<Runtime>;
}

impl pallet_utility::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeCall = RuntimeCall;
    type PalletsOrigin = OriginCaller;
    type WeightInfo = weights::pallet_utility::ZKVWeight<Runtime>;
}

mod vk_registration_parameters {
    use super::*;

    fn vks_key_size() -> u32 {
        Identity::max_len::<sp_core::H256>() as u32
    }
    fn tickets_key_size() -> u32 {
        Blake2_128Concat::max_len::<(AccountId, sp_core::H256)>() as u32
    }
    fn tickets_value_size() -> u32 {
        VkRegistrationHoldConsideration::max_encoded_len() as u32
    }
    parameter_types! {
        pub VkRegistrationBaseDeposit: Balance = currency::deposit(2, vks_key_size() + tickets_key_size() + tickets_value_size());
        pub const VkRegistrationByteDeposit: Balance = currency::deposit(0, 1);
        pub const VkRegistrationHoldReason: RuntimeHoldReason = RuntimeHoldReason::CommonVerifiers(pallet_verifiers::common::HoldReason::VkRegistration);
    }
}

/// Linear increment.
pub struct Linear<Base, Slope, Balance>(core::marker::PhantomData<(Base, Slope, Balance)>);
impl<Base, Slope> pallet_aggregate::ComputePublisherTip<Balance> for Linear<Base, Slope, Balance>
where
    Base: Get<Balance>,
    Slope: Get<Permill>,
{
    fn compute_tip(estimated: Balance) -> Option<Balance> {
        Base::get()
            .saturating_add(Slope::get().mul_floor(estimated))
            .into()
    }
}

impl DispatchAggregation<Balance, AccountId> for Runtime {
    fn dispatch_aggregation(
        _domain_id: u32,
        _aggregation_id: u64,
        _aggregation: H256,
        destination_params: Destination,
        _fee: Balance,
        _delivery_owner: AccountId,
    ) -> DispatchResult {
        match destination_params {
            Destination::None => Ok(()),
            Destination::Hyperbridge(_params) => Err(sp_runtime::DispatchError::Other(
                "Hyperbridge is not implemented yet",
            )),
        }
    }

    fn max_weight() -> Weight {
        Default::default()
    }

    fn dispatch_weight(_destination: &Destination) -> Weight {
        Default::default()
    }
}

parameter_types! {
    pub const AggregateDomainBaseDeposit: Balance = currency::deposit(2, 64);
    pub const AggregateDomainByteDeposit: Balance = currency::deposit(0, 1);
    pub const AggregateDomainHoldReason: RuntimeHoldReason = RuntimeHoldReason::Aggregate(pallet_aggregate::HoldReason::Domain);
    pub const AggregateBaseTip: Balance = 10 * CENTS;
    pub const AggregateLinearTip: Permill = Permill::from_percent(10);
    pub const AggregateMaxSize: pallet_aggregate::AggregationSize = 128;
    pub const AggregateQueueSize: u32 = 16;
    pub const AggregateAllowlistHoldBaseDeposit: Balance = currency::deposit(2, 0);
    // From KeyLenOf di double_map.rs in substrate.
    // k1.size + k2.size + 2 * Twox128.size = 4 + 32 + 2 * 16 = 68
    pub const AggregateAllowlistHoldSingleElementDeposit: Balance = currency::deposit(0, 68);
    pub const AggregateAllowlistHoldReason: RuntimeHoldReason = RuntimeHoldReason::Aggregate(pallet_aggregate::HoldReason::Allowlist);
}

/// A storage price that increases with the number of items in the storage but not consider the size of the items.
pub struct StoreItemsStoragePrice<Base, ItemPrice, Balance>(
    core::marker::PhantomData<(Base, ItemPrice, Balance)>,
);
impl<Base, ItemPrice, Balance> Convert<Footprint, Balance>
for StoreItemsStoragePrice<Base, ItemPrice, Balance>
where
    Base: Get<Balance>,
    ItemPrice: Get<Balance>,
    Balance: From<u64> + sp_runtime::Saturating,
{
    fn convert(a: Footprint) -> Balance {
        let s: Balance = a.count.into();
        s.saturating_mul(ItemPrice::get())
            .saturating_add(Base::get())
    }
}

impl pallet_aggregate::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type RuntimeHoldReason = RuntimeHoldReason;
    type AggregationSize = AggregateMaxSize;
    type MaxPendingPublishQueueSize = AggregateQueueSize;
    type ManagerOrigin = EnsureRoot<AccountId>;
    type Hold = Balances;

    type ConsiderationDomain = HoldConsideration<
        AccountId,
        Balances,
        AggregateDomainHoldReason,
        LinearStoragePrice<AggregateDomainBaseDeposit, AggregateDomainByteDeposit, Balance>,
    >;
    type ConsiderationAllowList = HoldConsideration<
        AccountId,
        Balances,
        AggregateAllowlistHoldReason,
        StoreItemsStoragePrice<
            AggregateAllowlistHoldBaseDeposit,
            AggregateAllowlistHoldSingleElementDeposit,
            Balance,
        >,
    >;
    type EstimateCallFee = TransactionPayment;

    type ComputePublisherTip = Linear<AggregateBaseTip, AggregateLinearTip, Balance>;

    type WeightInfo = weights::pallet_aggregate::ZKVWeight<Runtime>;

    #[cfg(feature = "runtime-benchmarks")]
    const AGGREGATION_SIZE: u32 = AggregateMaxSize::get() as u32;

    #[cfg(feature = "runtime-benchmarks")]
    type Currency = Balances;

    type DispatchAggregation = Self;
}

use vk_registration_parameters::*;

type VkRegistrationHoldConsideration = HoldConsideration<
    AccountId,
    Balances,
    VkRegistrationHoldReason,
    LinearStoragePrice<VkRegistrationBaseDeposit, VkRegistrationByteDeposit, Balance>,
>;

impl pallet_verifiers::common::Config for Runtime {
    type CommonWeightInfo = Runtime;
}

parameter_types! {
    pub const EzklMaxPubs: u32 = 32;
}

impl pallet_ezkl_verifier::Config for Runtime {
    type MaxPubs = EzklMaxPubs;
}

pub type EzklVerifier = pallet_ezkl_verifier::Ezkl<Runtime>;

impl pallet_verifiers::Config<EzklVerifier> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnProofVerified = Aggregate;
    type WeightInfo = pallet_ezkl_verifier::EzklWeight<weights::pallet_ezkl_verifier::ZKVWeight<Runtime>>;
    type Ticket = VkRegistrationHoldConsideration;
    #[cfg(feature = "runtime-benchmarks")]
    type Currency = Balances;
}

impl pallet_verifiers::Config<pallet_fflonk_verifier::Fflonk> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnProofVerified = Aggregate;
    type Ticket = VkRegistrationHoldConsideration;
    type WeightInfo = pallet_fflonk_verifier::FflonkWeight<weights::pallet_fflonk_verifier::ZKVWeight<Runtime>>;
    #[cfg(feature = "runtime-benchmarks")]
    type Currency = Balances;
}

pub const GROTH16_MAX_NUM_INPUTS: u32 = 64;
parameter_types! {
    pub const Groth16MaxNumInputs: u32 = GROTH16_MAX_NUM_INPUTS;
}

impl pallet_groth16_verifier::Config for Runtime {
    const MAX_NUM_INPUTS: u32 = Groth16MaxNumInputs::get();
}

// We should be sure that the max number of inputs does not exceed the max number of inputs in the verifier crate.
const_assert!(
    <Runtime as pallet_groth16_verifier::Config>::MAX_NUM_INPUTS
        <= pallet_groth16_verifier::MAX_NUM_INPUTS
);

impl pallet_verifiers::Config<pallet_groth16_verifier::Groth16<Runtime>> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnProofVerified = Aggregate;
    type Ticket = VkRegistrationHoldConsideration;
    type WeightInfo = pallet_groth16_verifier::Groth16Weight<weights::pallet_groth16_verifier::ZKVWeight<Runtime>>; // Mock
    #[cfg(feature = "runtime-benchmarks")]
    type Currency = Balances;
}

parameter_types! {
    pub const Plonky2MaxPubsSize: u32 = 512; // eq of 64 public inputs
    pub const Plonky2MaxProofSize: u32 = 262_144;
    pub const Plonky2MaxVkSize: u32 = 50_000;
}

impl pallet_plonky2_verifier::Config for Runtime {
    type MaxProofSize = Plonky2MaxProofSize;
    type MaxPubsSize = Plonky2MaxPubsSize;
    type MaxVkSize = Plonky2MaxVkSize;
    type WeightInfo = weights::pallet_plonky2_verifier_verify_proof::ZKVWeight<Runtime>;
}

impl pallet_verifiers::Config<pallet_plonky2_verifier::Plonky2<Runtime>> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnProofVerified = Aggregate;
    type Ticket = VkRegistrationHoldConsideration;
    type WeightInfo = pallet_plonky2_verifier::Plonky2Weight<weights::pallet_plonky2_verifier::ZKVWeight<Runtime>>;
    #[cfg(feature = "runtime-benchmarks")]
    type Currency = Balances;
}

pub const SP1_MAX_PUBS_SIZE: u32 = 32 * 64;
parameter_types! {
    pub const Sp1MaxPubsSize: u32 = SP1_MAX_PUBS_SIZE;
}

impl pallet_sp1_verifier::Config for Runtime {
    type MaxPubsSize = Sp1MaxPubsSize;
}

impl pallet_verifiers::Config<pallet_sp1_verifier::Sp1<Runtime>> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnProofVerified = Aggregate;
    type Ticket = VkRegistrationHoldConsideration;
    type WeightInfo = pallet_sp1_verifier::Sp1Weight<weights::pallet_sp1_verifier::ZKVWeight<Runtime>>;
    #[cfg(feature = "runtime-benchmarks")]
    type Currency = Balances;
}

parameter_types! {
    pub const Risc0MaxNSegment: u32 = 4;             // 4 segment of 2^20
    pub const Risc0Segment20MaxSize: u32 = 350_000; // risc0 2^20 segment size (a standard 2^22)
                                                    // proof is ~1_400_000
    pub const Risc0MaxPubsSize: u32 = 4 + 32 * 64;  // 4: bytes for payload length,
                                                    // 32 * 64: sufficient multiple of 32 bytes
}

impl pallet_risc0_verifier::Config for Runtime {
    type MaxNSegment = Risc0MaxNSegment;
    type Segment20MaxSize = Risc0Segment20MaxSize;
    type MaxPubsSize = Risc0MaxPubsSize;
    type WeightInfo = weights::pallet_risc0_verifier_verify_proof::ZKVWeight<Runtime>;
}

impl pallet_verifiers::Config<pallet_risc0_verifier::Risc0<Runtime>> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnProofVerified = Aggregate;
    type Ticket = VkRegistrationHoldConsideration;
    type WeightInfo = pallet_risc0_verifier::Risc0Weight<weights::pallet_risc0_verifier::ZKVWeight<Runtime>>;
    #[cfg(feature = "runtime-benchmarks")]
    type Currency = Balances;
}

parameter_types! {
    pub const UltrahonkMaxPubs: u32 = 32;
}

impl pallet_ultrahonk_verifier::Config for Runtime {
    type MaxPubs = UltrahonkMaxPubs;
}

pub type UltrahonkVerifier = pallet_ultrahonk_verifier::Ultrahonk<Runtime>;

impl pallet_verifiers::Config<UltrahonkVerifier> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnProofVerified = Aggregate;
    type Ticket = VkRegistrationHoldConsideration;
    type WeightInfo = pallet_ultrahonk_verifier::UltrahonkWeight<weights::pallet_ultrahonk_verifier::ZKVWeight<Runtime>>;
    #[cfg(feature = "runtime-benchmarks")]
    type Currency = Balances;
}

parameter_types! {
    pub const UltraplonkMaxPubs: u32 = 32;
}

impl pallet_ultraplonk_verifier::Config for Runtime {
    type MaxPubs = UltraplonkMaxPubs;
}

pub type UltraplonkVerifier = pallet_ultraplonk_verifier::Ultraplonk<Runtime>;

impl pallet_verifiers::Config<UltraplonkVerifier> for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type OnProofVerified = Aggregate;
    type Ticket = VkRegistrationHoldConsideration;
    type WeightInfo = pallet_ultraplonk_verifier::UltraplonkWeight<weights::pallet_ultraplonk_verifier::ZKVWeight<Runtime>>;
    #[cfg(feature = "runtime-benchmarks")]
    type Currency = Balances;
}

// Create the runtime by composing the FRAME pallets that were previously configured.
construct_runtime!(
    pub struct Runtime {
        // System support stuff.
        System: frame_system = 0,
        ParachainSystem: cumulus_pallet_parachain_system = 1,
        Timestamp: pallet_timestamp = 2,
        ParachainInfo: parachain_info = 3,
        Utility: pallet_utility = 4,

        // Monetary stuff.
        Balances: pallet_balances = 10,
        TransactionPayment: pallet_transaction_payment = 11,

        // Governance
        Sudo: pallet_sudo = 15,

        // Collator support. The order of these 4 are important and shall not change.
        Authorship: pallet_authorship = 20,
        CollatorSelection: pallet_collator_selection = 21,
        Session: pallet_session = 22,
        Aura: pallet_aura = 23,
        AuraExt: cumulus_pallet_aura_ext = 24,

        // XCM helpers.
        XcmpQueue: cumulus_pallet_xcmp_queue = 30,
        XcmPallet: pallet_xcm = 31,
        CumulusXcm: cumulus_pallet_xcm = 32,
        MessageQueue: pallet_message_queue = 33,

        // Our stuff
        Aggregate: pallet_aggregate = 81,

        // Verifiers. Start indices at 160 to leave room and to the end (255). Don't add
        // any kind of other pallets after this value.
        CommonVerifiers: pallet_verifiers::common = 160,
        SettlementGroth16Pallet: pallet_groth16_verifier = 161,
        SettlementRisc0Pallet: pallet_risc0_verifier = 162,
        SettlementUltraplonkPallet: pallet_ultraplonk_verifier = 163,
        SettlementPlonky2Pallet: pallet_plonky2_verifier = 165,
        SettlementFFlonkPallet: pallet_fflonk_verifier = 166,
        SettlementSp1Pallet: pallet_sp1_verifier = 167,
        SettlementUltrahonkPallet: pallet_ultrahonk_verifier = 168,
        SettlementEzklPallet: pallet_ezkl_verifier = 169,
    }
);

#[cfg(feature = "runtime-benchmarks")]
mod benches {
    frame_benchmarking::define_benchmarks!(
        [frame_system, SystemBench::<Runtime>]
        [pallet_balances, Balances]
        [pallet_session, SessionBench::<Runtime>]
        [pallet_utility, Utility]
        [pallet_timestamp, Timestamp]
        // [pallet_message_queue, MessageQueue]
        [pallet_sudo, Sudo]
        [pallet_collator_selection, CollatorSelection]
        [cumulus_pallet_parachain_system, ParachainSystem]
        // [cumulus_pallet_xcmp_queue, XcmpQueue]
        // our pallets
        // verifiers
        [pallet_groth16_verifier, Groth16VerifierBench::<Runtime>]
        [pallet_ezkl_verifier, EzklVerifierBench::<Runtime>]
        [pallet_fflonk_verifier, FflonkVerifierBench::<Runtime>]
        [pallet_risc0_verifier, Risc0VerifierBench::<Runtime>]
        [pallet_risc0_verifier_verify_proof, Risc0VerifierVerifyProofBench::<Runtime>]
        [pallet_risc0_verifier_extend, Risc0VerifierExtendBench::<Runtime>]
        [pallet_ultrahonk_verifier, UltrahonkVerifierBench::<Runtime>]
        [pallet_ultraplonk_verifier, UltraplonkVerifierBench::<Runtime>]
        [pallet_plonky2_verifier, Plonky2VerifierBench::<Runtime>]
        [pallet_plonky2_verifier_verify_proof, Plonky2VerifierVerifyProofBench::<Runtime>]
        [pallet_sp1_verifier, Sp1VerifierBench::<Runtime>]
    );
}

impl_runtime_apis! {
    impl sp_consensus_aura::AuraApi<Block, AuraId> for Runtime {
        fn slot_duration() -> sp_consensus_aura::SlotDuration {
            sp_consensus_aura::SlotDuration::from_millis(Aura::slot_duration())
        }

        fn authorities() -> Vec<AuraId> {
            Authorities::<Runtime>::get().into_inner()
        }
    }

    impl sp_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block)
        }

        fn initialize_block(header: &<Block as BlockT>::Header) -> sp_runtime::ExtrinsicInclusionMode {
            Executive::initialize_block(header)
        }
    }

    impl sp_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            OpaqueMetadata::new(Runtime::metadata().into())
        }

        fn metadata_at_version(version: u32) -> Option<OpaqueMetadata> {
            Runtime::metadata_at_version(version)
        }

        fn metadata_versions() -> Vec<u32> {
            Runtime::metadata_versions()
        }
    }

    impl sp_block_builder::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyExtrinsicResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: sp_inherents::InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(
            block: Block,
            data: sp_inherents::InherentData,
        ) -> sp_inherents::CheckInherentsResult {
            data.check_extrinsics(&block)
        }
    }

    impl sp_transaction_pool::runtime_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(
            source: TransactionSource,
            tx: <Block as BlockT>::Extrinsic,
            block_hash: <Block as BlockT>::Hash,
        ) -> TransactionValidity {
            Executive::validate_transaction(source, tx, block_hash)
        }
    }

    impl sp_offchain::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(header: &<Block as BlockT>::Header) {
            Executive::offchain_worker(header)
        }
    }

    impl sp_session::SessionKeys<Block> for Runtime {
        fn generate_session_keys(seed: Option<Vec<u8>>) -> Vec<u8> {
            SessionKeys::generate(seed)
        }

        fn decode_session_keys(
            encoded: Vec<u8>,
        ) -> Option<Vec<(Vec<u8>, KeyTypeId)>> {
            SessionKeys::decode_into_raw_public_keys(&encoded)
        }
    }

    impl frame_system_rpc_runtime_api::AccountNonceApi<Block, AccountId, Nonce> for Runtime {
        fn account_nonce(account: AccountId) -> Nonce {
            System::account_nonce(account)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentApi<Block, Balance> for Runtime {
        fn query_info(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment_rpc_runtime_api::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_info(uxt, len)
        }
        fn query_fee_details(
            uxt: <Block as BlockT>::Extrinsic,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_fee_details(uxt, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl pallet_transaction_payment_rpc_runtime_api::TransactionPaymentCallApi<Block, Balance, RuntimeCall>
        for Runtime
    {
        fn query_call_info(
            call: RuntimeCall,
            len: u32,
        ) -> pallet_transaction_payment::RuntimeDispatchInfo<Balance> {
            TransactionPayment::query_call_info(call, len)
        }
        fn query_call_fee_details(
            call: RuntimeCall,
            len: u32,
        ) -> pallet_transaction_payment::FeeDetails<Balance> {
            TransactionPayment::query_call_fee_details(call, len)
        }
        fn query_weight_to_fee(weight: Weight) -> Balance {
            TransactionPayment::weight_to_fee(weight)
        }
        fn query_length_to_fee(length: u32) -> Balance {
            TransactionPayment::length_to_fee(length)
        }
    }

    impl cumulus_primitives_core::CollectCollationInfo<Block> for Runtime {
        fn collect_collation_info(header: &<Block as BlockT>::Header) -> cumulus_primitives_core::CollationInfo {
            ParachainSystem::collect_collation_info(header)
        }
    }

    impl aggregate_rpc_runtime_api::AggregateApi<Block> for Runtime {
        fn get_statement_path(
            domain_id: u32,
            aggregation_id: u64,
            statement: sp_core::H256
        ) -> Result<aggregate_rpc_runtime_api::MerkleProof, aggregate_rpc_runtime_api::PathRequestError> {
            Aggregate::get_statement_path(domain_id, aggregation_id, statement).map(|c| c.into())
        }
    }

    #[cfg(feature = "try-runtime")]
    impl frame_try_runtime::TryRuntime<Block> for Runtime {
        fn on_runtime_upgrade(checks: frame_try_runtime::UpgradeCheckSelect) -> (Weight, Weight) {
            let weight = Executive::try_runtime_upgrade(checks).unwrap();
            (weight, RuntimeBlockWeights::get().max_block)
        }

        fn execute_block(
            block: Block,
            state_root_check: bool,
            signature_check: bool,
            select: frame_try_runtime::TryStateSelect,
        ) -> Weight {
            // NOTE: intentional unwrap: we don't want to propagate the error backwards, and want to
            // have a backtrace here.
            Executive::try_execute_block(block, state_root_check, signature_check, select).unwrap()
        }
    }

    #[cfg(feature = "runtime-benchmarks")]
    impl frame_benchmarking::Benchmark<Block> for Runtime {
        fn benchmark_metadata(extra: bool) -> (
            Vec<frame_benchmarking::BenchmarkList>,
            Vec<frame_support::traits::StorageInfo>,
        ) {
            use frame_benchmarking::{Benchmarking, BenchmarkList};
            use frame_support::traits::StorageInfoTrait;
            use frame_system_benchmarking::Pallet as SystemBench;
            use cumulus_pallet_session_benchmarking::Pallet as SessionBench;
            use pallet_groth16_verifier::benchmarking::Pallet as Groth16VerifierBench;
            use pallet_fflonk_verifier::benchmarking::Pallet as FflonkVerifierBench;
            use pallet_risc0_verifier::benchmarking::Pallet as Risc0VerifierBench;
            use pallet_risc0_verifier::benchmarking_verify_proof::Pallet as Risc0VerifierVerifyProofBench;
            use pallet_risc0_verifier::extend_benchmarking::Pallet as Risc0VerifierExtendBench;
            use pallet_ultrahonk_verifier::benchmarking::Pallet as UltrahonkVerifierBench;
            use pallet_ultraplonk_verifier::benchmarking::Pallet as UltraplonkVerifierBench;
            use pallet_plonky2_verifier::benchmarking_verify_proof::Pallet as Plonky2VerifierVerifyProofBench;
            use pallet_plonky2_verifier::benchmarking::Pallet as Plonky2VerifierBench;
            use pallet_sp1_verifier::benchmarking::Pallet as Sp1VerifierBench;
            use pallet_ezkl_verifier::benchmarking::Pallet as EzklVerifierBench;

            let mut list = Vec::<BenchmarkList>::new();
            list_benchmarks!(list, extra);

            let storage_info = AllPalletsWithSystem::storage_info();
            (list, storage_info)
        }

        fn dispatch_benchmark(
            config: frame_benchmarking::BenchmarkConfig
        ) -> Result<Vec<frame_benchmarking::BenchmarkBatch>, alloc::string::String> {
            use frame_benchmarking::{Benchmarking, BenchmarkBatch};

            use frame_system_benchmarking::Pallet as SystemBench;

            use cumulus_pallet_session_benchmarking::Pallet as SessionBench;

            use pallet_groth16_verifier::benchmarking::Pallet as Groth16VerifierBench;
            use pallet_ezkl_verifier::benchmarking::Pallet as EzklVerifierBench;
            use pallet_fflonk_verifier::benchmarking::Pallet as FflonkVerifierBench;
            use pallet_risc0_verifier::benchmarking::Pallet as Risc0VerifierBench;
            use pallet_risc0_verifier::benchmarking_verify_proof::Pallet as Risc0VerifierVerifyProofBench;
            use pallet_risc0_verifier::extend_benchmarking::Pallet as Risc0VerifierExtendBench;
            use pallet_ultrahonk_verifier::benchmarking::Pallet as UltrahonkVerifierBench;
            use pallet_ultraplonk_verifier::benchmarking::Pallet as UltraplonkVerifierBench;
            use pallet_plonky2_verifier::benchmarking_verify_proof::Pallet as Plonky2VerifierVerifyProofBench;
            use pallet_plonky2_verifier::benchmarking::Pallet as Plonky2VerifierBench;
            use pallet_sp1_verifier::benchmarking::Pallet as Sp1VerifierBench;

            use frame_support::traits::WhitelistedStorageKeys;
            let whitelist = AllPalletsWithSystem::whitelisted_storage_keys();

            let mut batches = Vec::<BenchmarkBatch>::new();
            let params = (&config, &whitelist);
            add_benchmarks!(params, batches);

            if batches.is_empty() { return Err("Benchmark not found for this pallet.".into()) }
            Ok(batches)
        }
    }

    impl sp_genesis_builder::GenesisBuilder<Block> for Runtime {
        fn build_state(config: Vec<u8>) -> sp_genesis_builder::Result {
            build_state::<RuntimeGenesisConfig>(config)
        }

        fn get_preset(id: &Option<sp_genesis_builder::PresetId>) -> Option<Vec<u8>> {
            get_preset::<RuntimeGenesisConfig>(id, |_| None)
        }

        fn preset_names() -> Vec<sp_genesis_builder::PresetId> {
            vec![]
        }
    }

    impl cumulus_primitives_aura::AuraUnincludedSegmentApi<Block> for Runtime {
        fn can_build_upon(
            included_hash: <Block as BlockT>::Hash,
            slot: cumulus_primitives_aura::Slot,
        ) -> bool {
            ConsensusHook::can_build_upon(included_hash, slot)
        }
    }
}

#[cfg(feature = "runtime-benchmarks")]
mod runtime_benchmarking_extra_config {
    use crate::Runtime;
    use alloc::vec::Vec;
    use frame_benchmarking::BenchmarkError;

    impl frame_system_benchmarking::Config for Runtime {
        fn setup_set_code_requirements(code: &Vec<u8>) -> Result<(), BenchmarkError> {
            crate::ParachainSystem::initialize_for_set_code_benchmark(code.len() as u32);
            Ok(())
        }

        fn verify_set_code() {
            crate::System::assert_last_event(
                cumulus_pallet_parachain_system::Event::<Runtime>::ValidationFunctionStored.into(),
            );
        }
    }

    impl cumulus_pallet_session_benchmarking::Config for Runtime {}
}

cumulus_pallet_parachain_system::register_validate_block! {
    Runtime = Runtime,
    BlockExecutor = cumulus_pallet_aura_ext::BlockExecutor::<Runtime, Executive>,
}
