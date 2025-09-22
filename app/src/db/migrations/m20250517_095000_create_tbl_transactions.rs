use super::m20250517_093000_create_tbl_users::TblUsers;
use super::m20250517_094000_create_tbl_wallets::TblWallets;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TblTransactions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TblTransactions::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TblTransactions::UserId).integer().not_null())
                    .col(
                        ColumnDef::new(TblTransactions::WalletId)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TblTransactions::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TblTransactions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_transaction_user_id")
                            .from(TblTransactions::Table, TblTransactions::UserId)
                            .to(TblUsers::Table, TblUsers::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_transaction_wallet_id")
                            .from(TblTransactions::Table, TblTransactions::WalletId)
                            .to(TblWallets::Table, TblWallets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_transaction_user_id")
                    .table(TblTransactions::Table)
                    .col(TblTransactions::UserId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_transaction_wallet_id")
                    .table(TblTransactions::Table)
                    .col(TblTransactions::WalletId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TblTransactions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum TblTransactions {
    Table,
    Id,
    UserId,
    WalletId,
    CreatedAt,
    UpdatedAt,
}
