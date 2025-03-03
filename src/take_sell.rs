use crate::app::App;
use mostro_core::order::SmallOrder;
use nostr_sdk::Client; // Add this line to import the App type

pub async fn take_sell(app: &mut App, order: SmallOrder, client: &Client) {
    println!("app: {:#?}", app);
    if app.show_amount_input {
        app.show_order = true;
        app.show_amount_input = false;
    }
}
