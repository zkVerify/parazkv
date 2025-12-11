use super::{
    AccountId, AllPalletsWithSystem, Balances, ParachainInfo, ParachainSystem, Runtime,
    RuntimeCall, RuntimeEvent, RuntimeOrigin, XcmPallet,
};
use crate::weights::pallet_xcm_benchmarks;
use crate::RuntimeBlockWeights;
use crate::TransactionByteFee;
use crate::{MessageQueue, Perbill, XcmpQueue};
use crate::{CENTIUNIT, MILLIUNIT};
use cumulus_primitives_core::{AggregateMessageOrigin, ParaId};
use frame_support::traits::Get;
use frame_support::traits::TransformOrigin;
use frame_support::{
    match_types, parameter_types,
    traits::tokens::imbalance::ResolveTo,
    traits::{ConstU32, Contains, Equals, Everything, Nothing, PalletInfoAccess},
    weights::Weight,
};
use frame_system::EnsureRoot;
use pallet_xcm::XcmPassthrough;
use parachains_common::{
    message_queue::{NarrowOriginToSibling, ParaIdToSibling},
    xcm_config::ConcreteAssetFromSystem,
};
use xcm::latest::prelude::*;
use xcm_builder::HashedDescription;
use xcm_builder::{
    AccountId32Aliases, AllowKnownQueryResponses, AllowSubscriptionsFrom,
    AllowTopLevelPaidExecutionFrom, DenyReserveTransferToRelayChain, DenyThenTry, EnsureXcmOrigin,
    FrameTransactionalProcessor, FungibleAdapter, IsConcrete, NativeAsset, ParentIsPreset,
    RelayChainAsNative, SiblingParachainAsNative, SignedAccountId32AsNative, SignedToAccountId32,
    SovereignSignedViaLocation, TakeWeightCredit, TrailingSetTopicAsId, UsingComponents,
    WeightInfoBounds, WithComputedOrigin, WithUniqueTopic, XcmFeeManagerFromComponents,
};
use xcm_builder::{DescribeAllTerminal, DescribeFamily, SendXcmFeeToAccount};
use xcm_executor::traits::ConvertLocation;
use xcm_executor::XcmExecutor;

pub(crate) const ZKV_GENESIS_HASH: [u8; 32] =
    hex_literal::hex!("ff7fe5a610f15fe7a0c52f94f86313fb7db7d3786e7f8acf2b66c11d5be7c242");

parameter_types! {
    pub const RootLocation: Location = Here.into_location();
    pub const RelayLocation: Location = Location::parent();
    pub const RelayNetwork: Option<NetworkId> = Some(NetworkId::ByGenesis(ZKV_GENESIS_HASH));
    pub BalancesPalletLocation: Location = PalletInstance(<Balances as PalletInfoAccess>::index() as u8).into();
    pub RelayChainOrigin: RuntimeOrigin = cumulus_pallet_xcm::Origin::Relay.into();
    pub UniversalLocation: InteriorLocation = [GlobalConsensus(RelayNetwork::get().unwrap()), Parachain(ParachainInfo::parachain_id().into())].into();
}

/// Type for specifying how a `Location` can be converted into an `AccountId`. This is used
/// when determining ownership of accounts for asset transacting and when attempting to use XCM
/// `Transact` in order to determine the dispatch Origin.
pub type LocationToAccountId = (
    // The parent (Relay-chain) origin converts to the parent `AccountId`.
    ParentIsPreset<AccountId>,
    // Straight up local `AccountId32` origins just alias directly to `AccountId`.
    AccountId32Aliases<RelayNetwork, AccountId>,
    // Extract `AccountId32` for account in the parent chain.
    AccountId32InParent<RelayNetwork, AccountId>,
    // Foreign locations alias into accounts according to a hash of their standard description
    // (e.g. remote origins).
    HashedDescription<AccountId, DescribeFamily<DescribeAllTerminal>>,
);

/// Extracts the `AccountId32` from the passed `location` if the network matches.
pub struct AccountId32InParent<Network, AccountId>(core::marker::PhantomData<(Network, AccountId)>);
impl<Network: Get<Option<NetworkId>>, AccountId: From<[u8; 32]> + Into<[u8; 32]> + Clone>
    ConvertLocation<AccountId> for AccountId32InParent<Network, AccountId>
{
    fn convert_location(location: &Location) -> Option<AccountId> {
        let id = match location.unpack() {
            // use account id only if originator is on the relay chain
            (1, [AccountId32 { id, network: None }]) => id,
            (1, [AccountId32 { id, network }]) if *network == Network::get() => id,
            _ => return None,
        };
        Some((*id).into())
    }
}

/// Means for transacting assets on this chain.
pub type LocalAssetTransactor = FungibleAdapter<
    // Use this currency:
    Balances,
    // Use this currency when it is a fungible asset matching the given location or name:
    IsConcrete<RelayLocation>,
    // Do a simple punn to convert an AccountId32 Location into a native chain account ID:
    LocationToAccountId,
    // Our chain's account ID type (we can't get away without mentioning it explicitly):
    AccountId,
    // We don't track any teleports.
    (), //LocalCheckAccount,
>;

/// Means for transacting assets on this chain.
pub type AssetTransactors = LocalAssetTransactor;

/// This is the type we use to convert an (incoming) XCM origin into a local `Origin` instance,
/// ready for dispatching a transaction with Xcm's `Transact`. There is an `OriginKind` which can
/// biases the kind of local `Origin` it will become.
pub type XcmOriginToTransactDispatchOrigin = (
    // Sovereign account converter; this attempts to derive an `AccountId` from the origin location
    // using `LocationToAccountId` and then turn that into the usual `Signed` origin. Useful for
    // foreign chains who want to have a local sovereign account on this chain which they control.
    SovereignSignedViaLocation<LocationToAccountId, RuntimeOrigin>,
    // Native converter for Relay-chain (Parent) location; will convert to a `Relay` origin when
    // recognized.
    RelayChainAsNative<RelayChainOrigin, RuntimeOrigin>,
    // Native converter for sibling Parachains; will convert to a `SiblingPara` origin when
    // recognized.
    SiblingParachainAsNative<cumulus_pallet_xcm::Origin, RuntimeOrigin>,
    // Native signed account converter; this just converts an `AccountId32` origin into a normal
    // `RuntimeOrigin::Signed` origin of the same 32-byte value.
    SignedAccountId32AsNative<RelayNetwork, RuntimeOrigin>,
    // VFlow has: SignedAccountKey20AsNative<RelayNetwork, RuntimeOrigin>,

    // Xcm origins can be represented natively under the Xcm pallet's Xcm origin.
    XcmPassthrough<RuntimeOrigin>,
);

// TODO: Try to have the same number of decimal digits and token name
parameter_types! {
    // One XCM operation is 1_000_000_000 weight - almost certainly a conservative estimate.
    pub UnitWeightCost: Weight = Weight::from_parts(1_000_000_000, 64 * 1024);
    pub const MaxInstructions: u32 = 30;
    pub const MaxAssetsIntoHolding: u32 = 64;
    pub StakingPot: AccountId = crate::CollatorSelection::account_id();
}

match_types! {
    pub type ParentOrParentsExecutivePlurality: impl Contains<Location> = {
        Location { parents: 1, interior: Here }
    //| Location { parents: 1, interior: X1(Plurality { id: BodyId::Executive, .. }) }
    };
}

pub struct ParentRelayChain;
impl Contains<Location> for ParentRelayChain {
    fn contains(location: &Location) -> bool {
        // match the relay chain and any account on it
        matches!(location.unpack(), (1, [..]))
    }
}

// TODO: Change barrier to charge
pub type Barrier = TrailingSetTopicAsId<
    DenyThenTry<
        DenyReserveTransferToRelayChain,
        (
            TakeWeightCredit,
            AllowKnownQueryResponses<XcmPallet>,
            WithComputedOrigin<
                (AllowTopLevelPaidExecutionFrom<ParentRelayChain>,),
                UniversalLocation,
                ConstU32<8>,
            >,
            AllowSubscriptionsFrom<ParentRelayChain>,
        ),
    >,
>;

pub type TrustedTeleporters = ConcreteAssetFromSystem<RelayLocation>;

pub type WaivedLocations = (Equals<RelayLocation>, Equals<RootLocation>);

pub struct UnsafeCallFilter;
impl frame_support::traits::Contains<RuntimeCall> for UnsafeCallFilter {
    fn contains(_call: &RuntimeCall) -> bool {
        true
    }
}

pub struct XcmConfig;
impl xcm_executor::Config for XcmConfig {
    type RuntimeCall = RuntimeCall;
    type XcmSender = XcmRouter;
    // How to withdraw and deposit an asset.
    type AssetTransactor = AssetTransactors;
    type OriginConverter = XcmOriginToTransactDispatchOrigin;
    type IsReserve = NativeAsset;
    type IsTeleporter = TrustedTeleporters;
    type Aliasers = Nothing;
    type UniversalLocation = UniversalLocation;
    type Barrier = Barrier;
    type Weigher = WeightInfoBounds<
        pallet_xcm_benchmarks::ZKVWeight<RuntimeCall>,
        RuntimeCall,
        MaxInstructions,
    >;
    // Can only buy weight with the native token
    type Trader = UsingComponents<
        <Runtime as pallet_transaction_payment::Config>::WeightToFee,
        RelayLocation,
        AccountId,
        Balances,
        ResolveTo<StakingPot, Balances>,
    >;
    type ResponseHandler = XcmPallet;
    type AssetTrap = XcmPallet;
    type AssetLocker = ();
    type AssetExchanger = ();
    type AssetClaims = XcmPallet;
    type SubscriptionService = XcmPallet;
    type PalletInstancesInfo = AllPalletsWithSystem;
    type MaxAssetsIntoHolding = MaxAssetsIntoHolding;
    type FeeManager = XcmFeeManagerFromComponents<
        WaivedLocations,
        SendXcmFeeToAccount<AssetTransactors, StakingPot>,
    >;
    type MessageExporter = ();
    type UniversalAliases = Nothing;
    type CallDispatcher = RuntimeCall;
    type SafeCallFilter = UnsafeCallFilter;
    type TransactionalProcessor = FrameTransactionalProcessor;
    type HrmpNewChannelOpenRequestHandler = ();
    type HrmpChannelAcceptedHandler = ();
    type HrmpChannelClosingHandler = ();
    type XcmRecorder = XcmPallet;
}

// Converts a Signed Local Origin into a Location
pub type LocalOriginToLocation = SignedToAccountId32<RuntimeOrigin, AccountId, RelayNetwork>;

parameter_types! {
    /// The asset ID for the asset that we use to pay for message delivery fees.
    pub NativeAssetId: AssetId = AssetId(RelayLocation::get()); // the relay chain native asset
    pub FeeAssetId: AssetId = NativeAssetId::get();
    /// The base fee for the message delivery fees.
    pub const ToParentBaseDeliveryFee: u128 = MILLIUNIT.saturating_mul(3);
}

pub type PriceForParentDelivery = polkadot_runtime_common::xcm_sender::ExponentialPrice<
    FeeAssetId,
    ToParentBaseDeliveryFee,
    TransactionByteFee,
    ParachainSystem,
>;

/// The means for routing XCM messages which are not for local execution into
/// the right message queues.
pub type XcmRouter = WithUniqueTopic<(
    // Two routers - use UMP to communicate with the relay chain:
    cumulus_primitives_utility::ParentAsUmp<ParachainSystem, XcmPallet, PriceForParentDelivery>,
    // ..and XCMP to communicate with the sibling chains.
    XcmpQueue,
)>;

parameter_types! {
    pub const MaxLockers: u32 = 8;
    pub const MaxRemoteLockConsumers: u32 = 0;
}

impl pallet_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type Currency = Balances;
    type CurrencyMatcher = ();
    type SendXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmRouter = XcmRouter;
    type ExecuteXcmOrigin = EnsureXcmOrigin<RuntimeOrigin, LocalOriginToLocation>;
    type XcmExecuteFilter = Everything;
    type XcmExecutor = XcmExecutor<XcmConfig>;
    type XcmTeleportFilter = Everything;
    type XcmReserveTransferFilter = Nothing;
    type Weigher = WeightInfoBounds<
        pallet_xcm_benchmarks::ZKVWeight<RuntimeCall>,
        RuntimeCall,
        MaxInstructions,
    >;
    type UniversalLocation = UniversalLocation;
    type RuntimeOrigin = RuntimeOrigin;
    type RuntimeCall = RuntimeCall;
    const VERSION_DISCOVERY_QUEUE_SIZE: u32 = 100;
    // ^ Override for AdvertisedXcmVersion default
    type AdvertisedXcmVersion = pallet_xcm::CurrentXcmVersion;
    type AdminOrigin = EnsureRoot<AccountId>;
    type TrustedLockers = ();
    type SovereignAccountOf = LocationToAccountId;
    type MaxLockers = MaxLockers;
    type MaxRemoteLockConsumers = MaxLockers;
    type RemoteLockConsumerIdentifier = ();

    type WeightInfo = crate::weights::pallet_xcm::ZKVEvmWeight<Runtime>;
}

impl cumulus_pallet_xcm::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type XcmExecutor = XcmExecutor<XcmConfig>;
}

parameter_types! {
    pub MessageQueueServiceWeight: Weight = Perbill::from_percent(25) * RuntimeBlockWeights::get().max_block;
    pub const HeapSize: u32 = 103 * 1024;
    pub const MaxStale: u32 = 8;
}

impl pallet_message_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type WeightInfo = crate::weights::pallet_message_queue::ZKVEvmWeight<Runtime>;
    #[cfg(not(feature = "runtime-benchmarks"))]
    type MessageProcessor = xcm_builder::ProcessXcmMessage<
        AggregateMessageOrigin,
        xcm_executor::XcmExecutor<XcmConfig>,
        RuntimeCall,
    >;
    #[cfg(feature = "runtime-benchmarks")]
    type MessageProcessor =
        pallet_message_queue::mock_helpers::NoopMessageProcessor<AggregateMessageOrigin>;

    type Size = u32;
    // The XCMP queue pallet is only ever able to handle the `Sibling(ParaId)` origin:
    type QueueChangeHandler = NarrowOriginToSibling<XcmpQueue>;
    type QueuePausedQuery = NarrowOriginToSibling<XcmpQueue>;
    type HeapSize = HeapSize;
    type MaxStale = MaxStale;
    type ServiceWeight = MessageQueueServiceWeight;
    type IdleMaxServiceWeight = MessageQueueServiceWeight;
}

parameter_types! {
    pub const MaxInboundSuspended: u32 = 1000;
    /// The base fee for the message delivery fees. zkVerify is based for the reference.
    pub const ToSiblingBaseDeliveryFee: u128 = CENTIUNIT.saturating_mul(3);
}

/// Price For Sibling Parachain Delivery
type PriceForSiblingParachainDelivery = polkadot_runtime_common::xcm_sender::ExponentialPrice<
    FeeAssetId,
    ToSiblingBaseDeliveryFee,
    TransactionByteFee,
    XcmpQueue,
>;

impl cumulus_pallet_xcmp_queue::Config for Runtime {
    type RuntimeEvent = RuntimeEvent;
    type ChannelInfo = ParachainSystem;
    type VersionWrapper = ();
    // Enqueue XCMP messages from siblings for later processing.
    type XcmpQueue = TransformOrigin<MessageQueue, AggregateMessageOrigin, ParaId, ParaIdToSibling>;
    type MaxInboundSuspended = MaxInboundSuspended;
    type MaxActiveOutboundChannels = ConstU32<128>;
    // from polkadot-sdk:
    // Most on-chain HRMP channels are configured to use 102400 bytes of max message size, so we
    // need to set the page size larger than that until we reduce the channel size on-chain.
    type MaxPageSize = ConstU32<{ 103 * 1024 }>;
    type ControllerOrigin = EnsureRoot<AccountId>;
    type ControllerOriginConverter = XcmOriginToTransactDispatchOrigin;
    type PriceForSiblingDelivery = PriceForSiblingParachainDelivery;
    type WeightInfo = crate::weights::cumulus_pallet_xcmp_queue::ZKVEvmWeight<Runtime>;
}
