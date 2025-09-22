use crate::db::models::{WalletActiveModel, WalletColumn, WalletEntity, WalletModel};
use anyhow::Result;
use sea_orm::DeleteResult;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait,
    QueryFilter,
};

pub enum DbExecutor<'a> {
    #[allow(dead_code)]
    Connection(&'a DatabaseConnection),
    Transaction(&'a DatabaseTransaction),
}

pub struct WalletRepository<'a> {
    executor: DbExecutor<'a>,
}

impl<'a> WalletRepository<'a> {
    #[allow(dead_code)]
    pub fn new_with_connection(db: &'a DatabaseConnection) -> Self {
        Self {
            executor: DbExecutor::Connection(db),
        }
    }

    pub fn new_with_transaction(txn: &'a DatabaseTransaction) -> Self {
        Self {
            executor: DbExecutor::Transaction(txn),
        }
    }

    pub async fn find_by_id(&self, id: i32) -> Result<Option<WalletModel>> {
        match &self.executor {
            DbExecutor::Connection(db) => Ok(WalletEntity::find_by_id(id).one(*db).await?),
            DbExecutor::Transaction(txn) => Ok(WalletEntity::find_by_id(id).one(*txn).await?),
        }
    }

    #[allow(dead_code)]
    pub async fn find_by_user_id(&self, user_id: i32) -> Result<Vec<WalletModel>> {
        match &self.executor {
            DbExecutor::Connection(db) => Ok(WalletEntity::find()
                .filter(WalletColumn::UserId.eq(user_id))
                .all(*db)
                .await?),
            DbExecutor::Transaction(txn) => Ok(WalletEntity::find()
                .filter(WalletColumn::UserId.eq(user_id))
                .all(*txn)
                .await?),
        }
    }

    pub async fn create(&self, model: WalletActiveModel) -> Result<WalletModel> {
        match &self.executor {
            DbExecutor::Connection(db) => Ok(model.insert(*db).await?),
            DbExecutor::Transaction(txn) => Ok(model.insert(*txn).await?),
        }
    }

    pub async fn delete(&self, id: i32) -> Result<DeleteResult> {
        match &self.executor {
            DbExecutor::Connection(db) => Ok(WalletEntity::delete_by_id(id).exec(*db).await?),
            DbExecutor::Transaction(txn) => Ok(WalletEntity::delete_by_id(id).exec(*txn).await?),
        }
    }
}
