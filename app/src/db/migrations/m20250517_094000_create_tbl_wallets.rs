use super::m20250517_093000_create_tbl_users::TblUsers;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(TblWallets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TblWallets::Id)
                            .integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(TblWallets::UserId).integer().not_null())
                    .col(ColumnDef::new(TblWallets::Name).string().not_null())
                    .col(
                        ColumnDef::new(TblWallets::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TblWallets::UpdatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::current_timestamp())
                            .not_null(),
                    )
                    .col(ColumnDef::new(TblWallets::Chain).string().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_wallet_user_id")
                            .from(TblWallets::Table, TblWallets::UserId)
                            .to(TblUsers::Table, TblUsers::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_wallet_user_id")
                    .table(TblWallets::Table)
                    .col(TblWallets::UserId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(TblWallets::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum TblWallets {
    Table,
    Id,
    UserId,
    Name,
    CreatedAt,
    UpdatedAt,
    Chain,
}
