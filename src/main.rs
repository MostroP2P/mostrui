pub mod app;
pub mod db;
pub mod settings;
pub mod take_buy;
pub mod take_sell;
pub mod util;
pub mod widgets;

use crate::app::App;
use crate::db::{connect, User};
use crate::settings::{get_settings_path, init_global_settings, Settings};
use std::str::FromStr;

use mostro_core::NOSTR_REPLACEABLE_EVENT_KIND;
use nostr_sdk::prelude::*;
use std::path::PathBuf;
use std::sync::OnceLock;

static SETTINGS: OnceLock<Settings> = OnceLock::new();

#[tokio::main]
async fn main() -> Result<()> {
    let settings_path = get_settings_path();
    let settings_file_path = PathBuf::from(settings_path);

    // Create config global var
    init_global_settings(Settings::new(settings_file_path)?);
    let terminal = ratatui::init();
    let mostro = PublicKey::from_str(Settings::get().mostro_pubkey.as_str())?;
    let pool = connect().await?;

    let identity_keys = User::get_identity_keys(&pool)
        .await
        .map_err(|e| format!("Failed to get identity keys: {}", e))?;

    let (trade_keys, trade_index) = User::get_next_trade_keys(&pool)
        .await
        .map_err(|e| format!("Failed to get trade keys: {}", e))?;
    let user = User::get(&pool).await?;
    let app = App::new(
        mostro,
        identity_keys,
        trade_keys,
        trade_index,
        user.mnemonic,
    );
    // Call function to connect to relays
    let client = util::connect_nostr().await?;

    let since_time = chrono::Utc::now()
        .checked_sub_signed(chrono::Duration::days(7))
        .unwrap()
        .timestamp() as u64;
    let timestamp = Timestamp::from(since_time);

    let filters = Filter::new()
        .author(mostro)
        .limit(20)
        .since(timestamp)
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Y), vec!["mostro"])
        .custom_tag(SingleLetterTag::lowercase(Alphabet::Z), vec!["order"])
        .kind(Kind::Custom(NOSTR_REPLACEABLE_EVENT_KIND));
    // Here subscribe to get orders
    let orders_sub_id = SubscriptionId::new("orders-sub-id");
    client
        .subscribe_with_id(orders_sub_id, vec![filters], None)
        .await?;

    // Here subscribe to get messages
    // let messages_sub_id = SubscriptionId::new("messages-sub-id");
    // let filter = Filter::new()
    //     .pubkey(app.my_keys.public_key())
    //     .kinds([Kind::GiftWrap, Kind::PrivateDirectMessage])
    //     .since(timestamp);
    // client
    //     .subscribe_with_id(messages_sub_id, vec![filter], None)
    //     .await?;
    let app_result = app.run(terminal, client).await;
    ratatui::restore();

    app_result
}
