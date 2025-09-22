pub use sea_orm_migration::prelude::*;

mod m20250517_093000_create_tbl_users;
mod m20250517_094000_create_tbl_wallets;
mod m20250517_095000_create_tbl_transactions;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20250517_093000_create_tbl_users::Migration),
            Box::new(m20250517_094000_create_tbl_wallets::Migration),
            Box::new(m20250517_095000_create_tbl_transactions::Migration),
        ]
    }
}
