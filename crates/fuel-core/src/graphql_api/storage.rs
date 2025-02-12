use crate::{
    fuel_core_graphql_api::storage::{
        blocks::FuelBlockIdsToHeights,
        coins::OwnedCoins,
        messages::OwnedMessageIds,
        transactions::{
            OwnedTransactionIndexKey,
            OwnedTransactions,
            TransactionStatuses,
        },
    },
    graphql_api::ports::worker::OffChainDatabase,
};
use fuel_core_storage::{
    kv_store::{
        KeyValueInspect,
        StorageColumn,
    },
    transactional::{
        Modifiable,
        StorageTransaction,
    },
    Error as StorageError,
    Result as StorageResult,
    StorageAsMut,
    StorageMutate,
};
use fuel_core_types::{
    fuel_tx::{
        Address,
        Bytes32,
    },
    fuel_types::BlockHeight,
    services::txpool::TransactionStatus,
};
use statistic::StatisticTable;

pub mod blocks;
pub mod coins;
pub mod messages;
pub mod statistic;
pub mod transactions;

/// GraphQL database tables column ids to the corresponding [`fuel_core_storage::Mappable`] table.
#[repr(u32)]
#[derive(
    Copy,
    Clone,
    Debug,
    strum_macros::EnumCount,
    strum_macros::IntoStaticStr,
    PartialEq,
    Eq,
    enum_iterator::Sequence,
    Hash,
)]
pub enum Column {
    /// The column id of metadata about the blockchain
    Metadata = 0,
    /// The column of the table that stores `true` if `owner` owns `Coin` with `coin_id`
    OwnedCoins = 1,
    /// Transaction id to current status
    TransactionStatus = 2,
    /// The column of the table of all `owner`'s transactions
    TransactionsByOwnerBlockIdx = 3,
    /// The column of the table that stores `true` if `owner` owns `Message` with `message_id`
    OwnedMessageIds = 4,
    /// The column of the table that stores statistic about the blockchain.
    Statistic = 5,
    /// See [`blocks::FuelBlockIdsToHeights`]
    FuelBlockIdsToHeights = 6,
}

impl Column {
    /// The total count of variants in the enum.
    pub const COUNT: usize = <Self as strum::EnumCount>::COUNT;

    /// Returns the `usize` representation of the `Column`.
    pub fn as_u32(&self) -> u32 {
        *self as u32
    }
}

impl StorageColumn for Column {
    fn name(&self) -> &'static str {
        self.into()
    }

    fn id(&self) -> u32 {
        self.as_u32()
    }
}

impl<S> OffChainDatabase for StorageTransaction<S>
where
    S: KeyValueInspect<Column = Column> + Modifiable,
    StorageTransaction<S>: StorageMutate<OwnedMessageIds, Error = StorageError>
        + StorageMutate<OwnedCoins, Error = StorageError>
        + StorageMutate<FuelBlockIdsToHeights, Error = StorageError>,
{
    fn record_tx_id_owner(
        &mut self,
        owner: &Address,
        block_height: BlockHeight,
        tx_idx: u16,
        tx_id: &Bytes32,
    ) -> StorageResult<Option<Bytes32>> {
        self.storage::<OwnedTransactions>().insert(
            &OwnedTransactionIndexKey::new(owner, block_height, tx_idx),
            tx_id,
        )
    }

    fn update_tx_status(
        &mut self,
        id: &Bytes32,
        status: TransactionStatus,
    ) -> StorageResult<Option<TransactionStatus>> {
        self.storage::<TransactionStatuses>().insert(id, &status)
    }

    fn increase_tx_count(&mut self, new_txs_count: u64) -> StorageResult<u64> {
        /// Tracks the total number of transactions written to the chain
        /// It's useful for analyzing TPS or other metrics.
        const TX_COUNT: &str = "total_tx_count";

        // TODO: how should tx count be initialized after regenesis?
        let current_tx_count: u64 = self
            .storage::<StatisticTable<u64>>()
            .get(TX_COUNT)?
            .unwrap_or_default()
            .into_owned();
        // Using saturating_add because this value doesn't significantly impact the correctness of execution.
        let new_tx_count = current_tx_count.saturating_add(new_txs_count);
        <_ as StorageMutate<StatisticTable<u64>>>::insert(self, TX_COUNT, &new_tx_count)?;
        Ok(new_tx_count)
    }

    fn commit(self) -> StorageResult<()> {
        self.commit()?;
        Ok(())
    }
}
