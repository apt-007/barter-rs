use crate::{
    asset::Asset,
    instrument::{
        kind::InstrumentKind,
        market_data::{kind::MarketDataInstrumentKind, MarketDataInstrument},
        name::{InstrumentNameExchange, InstrumentNameInternal},
        spec::{InstrumentSpec, InstrumentSpecQuantity, OrderQuantityUnits},
    },
    Underlying,
};
use derive_more::{Constructor, Display};
use serde::{Deserialize, Serialize};
use std::fmt::Formatter;

/// Defines an [`Instrument`]s [`InstrumentKind`] (eg/ Spot, Perpetual, etc).
pub mod kind;

/// Defines the [`InstrumentNameExchange`] and [`InstrumentNameExchange`] types, used as
/// `SmolStr` identifiers for an [`Instrument`].
pub mod name;

/// Defines the [`InstrumentSpec`], including specifications for an [`Instrument`]s
/// price, quantity and notional value.
///
/// eg/ `InstrumentSpecPrice.tick_size`, `OrderQuantityUnits`, etc.
pub mod spec;

/// Defines a simplified [`MarketDataInstrument`], with only the necessary data to subscribe to
/// market data feeds.
pub mod market_data;

/// Unique identifier for an `Instrument` traded on an execution.
///
/// Used to key data events in a memory efficient way.
#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Display,
)]
pub struct InstrumentId(pub u64);

#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize, Constructor,
)]
pub struct InstrumentIndex(pub usize);

impl InstrumentIndex {
    pub fn index(&self) -> usize {
        self.0
    }
}

impl Display for InstrumentIndex {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "InstrumentIndex({})", self.0)
    }
}

/// Comprehensive Instrument model, containing all the data required to subscribe to market data
/// and generate correct orders.
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
pub struct Instrument<ExchangeKey, AssetKey> {
    pub exchange: ExchangeKey,
    pub name_internal: InstrumentNameInternal,
    pub name_exchange: InstrumentNameExchange,
    pub underlying: Underlying<AssetKey>,
    #[serde(alias = "instrument_kind")]
    pub kind: InstrumentKind<AssetKey>,
    pub spec: Option<InstrumentSpec<AssetKey>>,
}

impl<ExchangeKey, AssetKey> Instrument<ExchangeKey, AssetKey> {
    /// Construct a new [`Self`] with the provided data, assuming the [`InstrumentNameInternal`]
    /// can be created via the [`InstrumentNameInternal::new_from_exchange`] constructor.
    pub fn new<NameInternal, NameExchange>(
        exchange: ExchangeKey,
        name_internal: NameInternal,
        name_exchange: NameExchange,
        underlying: Underlying<AssetKey>,
        kind: InstrumentKind<AssetKey>,
        spec: Option<InstrumentSpec<AssetKey>>,
    ) -> Self
    where
        NameInternal: Into<InstrumentNameInternal>,
        NameExchange: Into<InstrumentNameExchange>,
    {
        Self {
            exchange,
            name_internal: name_internal.into(),
            name_exchange: name_exchange.into(),
            underlying,
            kind,
            spec,
        }
    }

    /// Map this Instruments `ExchangeKey` to a new key.
    pub fn map_exchange_key<NewExchangeKey>(
        self,
        exchange: NewExchangeKey,
    ) -> Instrument<NewExchangeKey, AssetKey> {
        let Instrument {
            exchange: _,
            name_internal,
            name_exchange,
            underlying,
            kind,
            spec,
        } = self;

        Instrument {
            exchange,
            name_internal,
            name_exchange,
            underlying,
            kind,
            spec,
        }
    }

    /// Map this Instruments `AssetKey` to a new key, using the provided lookup closure.
    pub fn map_asset_key_with_lookup<FnFindAsset, NewAssetKey, Error>(
        self,
        find_asset: FnFindAsset,
    ) -> Result<Instrument<ExchangeKey, NewAssetKey>, Error>
    where
        FnFindAsset: Fn(&AssetKey) -> Result<NewAssetKey, Error>,
    {
        let Instrument {
            exchange,
            name_internal,
            name_exchange,
            underlying: Underlying { base, quote },
            kind,
            spec,
        } = self;

        let base_new_key = find_asset(&base)?;
        let quote_new_key = find_asset(&quote)?;

        let kind = match kind {
            InstrumentKind::Spot => InstrumentKind::Spot,
            InstrumentKind::Perpetual { settlement_asset } => InstrumentKind::Perpetual {
                settlement_asset: find_asset(&settlement_asset)?,
            },
            InstrumentKind::Future {
                settlement_asset,
                contract,
            } => InstrumentKind::Future {
                settlement_asset: find_asset(&settlement_asset)?,
                contract,
            },
            InstrumentKind::Option {
                settlement_asset,
                contract,
            } => InstrumentKind::Option {
                settlement_asset: find_asset(&settlement_asset)?,
                contract,
            },
        };

        let spec = match spec {
            Some(spec) => {
                let InstrumentSpec {
                    price,
                    quantity:
                        InstrumentSpecQuantity {
                            unit,
                            min,
                            increment,
                        },
                    notional,
                } = spec;

                let unit = match unit {
                    OrderQuantityUnits::Asset(asset) => {
                        OrderQuantityUnits::Asset(find_asset(&asset)?)
                    }
                    OrderQuantityUnits::Contract => OrderQuantityUnits::Contract,
                    OrderQuantityUnits::Quote => OrderQuantityUnits::Quote,
                };

                Some(InstrumentSpec {
                    price,
                    quantity: InstrumentSpecQuantity {
                        unit,
                        min,
                        increment,
                    },
                    notional,
                })
            }
            None => None,
        };

        Ok(Instrument {
            exchange,
            name_internal,
            name_exchange,
            underlying: Underlying::new(base_new_key, quote_new_key),
            kind,
            spec,
        })
    }
}

impl<ExchangeKey> From<&Instrument<ExchangeKey, Asset>> for MarketDataInstrument {
    fn from(value: &Instrument<ExchangeKey, Asset>) -> Self {
        Self {
            base: value.underlying.base.name_internal.clone(),
            quote: value.underlying.quote.name_internal.clone(),
            kind: MarketDataInstrumentKind::from(&value.kind),
        }
    }
}
