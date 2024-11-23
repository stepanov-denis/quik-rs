//! # It works with exchange data from the PostgreSQL DBMS.
use crate::signal::CrossoverSignal;
use bb8::RunError;
use bb8_postgres::{bb8::Pool, tokio_postgres::NoTls, PostgresConnectionManager};
use chrono::{DateTime, Local};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use tracing::error;

#[derive(Debug)]
pub struct DataForEma {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

pub struct Db {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

/// Trading instruments
pub struct Instruments {
    // class_code: String,
    // instrument_status: String,
    pub sec_code: String,
    // instr_short_name: String,
    pub crossover_signal: CrossoverSignal,
}

impl Db {
    /// Initial the connection pool
    pub async fn new(
        connection_str: &str,
    ) -> Result<Self, RunError<bb8_postgres::tokio_postgres::Error>> {
        // Create the manager
        let manager = PostgresConnectionManager::new_from_stringlike(connection_str, NoTls)
            .map_err(|e| {
                error!("error creating PostgresConnectionManager: {:?}", e);
                e
            })?;

        // Build the connection pool
        let pool = Pool::builder()
            .max_size(15)
            .min_idle(Some(1))
            .build(manager)
            .await
            .map_err(|e| {
                error!("error building connection pool: {:?}", e);
                e
            })?;

        Ok(Db { pool })
    }

    /// Creating a table of current trades
    pub async fn create_current_trades(
        &self,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        // Create table
        let query = "
            CREATE TABLE IF NOT EXISTS current_trades (
                instrument_class VARCHAR(200),
                class VARCHAR(128),
                class_code VARCHAR(12),
                instr_short_name VARCHAR(20),
                sec_code VARCHAR(12),
                session_status VARCHAR(32),
                instrument_status VARCHAR(32),
                lot_multiplier INTEGER,
                lot_size INTEGER,
                last_price  DECIMAL(15,6),
                last_volume DECIMAL(15,6),
                last_price_time TIME,
                trade_date DATE
            );
        ";

        // Executing the command to create a table
        conn.execute(query, &[]).await?;

        Ok(())
    }

    /// Creating a table of historical trading data
    pub async fn create_historical_trades(
        &self,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        // Create table
        let query = "
            CREATE TABLE IF NOT EXISTS historical_trades (
                id SERIAL PRIMARY KEY,
                instrument_class VARCHAR(200),
                class VARCHAR(128),
                class_code VARCHAR(12),
                instr_short_name VARCHAR(20),
                sec_code VARCHAR(12),
                session_status VARCHAR(32),
                instrument_status VARCHAR(32),
                lot_multiplier INTEGER,
                lot_size INTEGER,
                last_price DECIMAL(15,6),
                last_volume DECIMAL(15,6),
                last_price_time TIME,
                trade_date DATE,
                update_timestamptz TIMESTAMPTZ DEFAULT NOW()
            );
        ";

        // Executing the command to create a table
        conn.execute(query, &[]).await?;

        Ok(())
    }

    /// Trigger function that will insert data into the historical_trades table
    pub async fn insert_into_historical(
        &self,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        // Creating a trigger function
        let query = "
            CREATE OR REPLACE FUNCTION insert_into_historical()
            RETURNS TRIGGER AS $$
            BEGIN
                INSERT INTO historical_trades (
                    instrument_class,
                    class,
                    class_code,
                    instr_short_name,
                    sec_code,
                    session_status,
                    instrument_status,
                    lot_multiplier,
                    lot_size,
                    last_price,
                    last_volume,
                    last_price_time,
                    trade_date
                ) VALUES (
                    NEW.instrument_class,
                    NEW.class,
                    NEW.class_code,
                    NEW.instr_short_name,
                    NEW.sec_code,
                    NEW.session_status,
                    NEW.instrument_status,
                    NEW.lot_multiplier,
                    NEW.lot_size,
                    NEW.last_price,
                    NEW.last_volume,
                    NEW.last_price_time,
                    NEW.trade_date
                );
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
        ";

        // Executing the command to create a trigger function
        conn.execute(query, &[]).await?;

        Ok(())
    }

    /// The trigger that will be triggered before updating the data in the current_trades table
    pub async fn before_update_current_trades(
        &self,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        // Create a trigger
        let query = "
            DO $$
            BEGIN
                IF NOT EXISTS (
                    SELECT 1 FROM pg_trigger
                    WHERE tgname = 'before_update_current_trades'
                ) THEN
                    CREATE TRIGGER before_update_current_trades
                    BEFORE UPDATE ON current_trades
                    FOR EACH ROW
                    EXECUTE FUNCTION insert_into_historical();
                END IF;
            END;
            $$ LANGUAGE plpgsql;
        ";

        // Executing the trigger creation command
        conn.execute(query, &[]).await?;

        Ok(())
    }

    // Инициализация базы данных
    pub async fn init(&self) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        let create_current_trades = self.create_current_trades().await;
        match create_current_trades {
            Ok(_) => {}
            Err(e) => error!("create table current_trades error: {}", e),
        }

        let create_historical_trades = self.create_historical_trades().await;
        match create_historical_trades {
            Ok(_) => {}
            Err(e) => error!("create table historical_trades error: {}", e),
        }

        let insert_into_historical = self.insert_into_historical().await;
        match insert_into_historical {
            Ok(_) => {}
            Err(e) => error!("create function insert_into_historical error: {}", e),
        }

        let before_update_current_trades = self.before_update_current_trades().await;
        match before_update_current_trades {
            Ok(_) => {}
            Err(e) => error!("create trigger before_update_current_trades error: {}", e),
        }

        Ok(())
    }

    pub async fn get_instruments(
        &self,
        class_code: &str,
        instrument_status: &str,
    ) -> Result<Vec<Instruments>, RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        let query = "
        SELECT DISTINCT ON (sec_code)
            class_code, instrument_status, sec_code, instr_short_name, update_timestamptz
        FROM historical_trades
        WHERE class_code = $1
          AND instrument_status = $2
        ORDER BY sec_code, update_timestamptz DESC
    ";

        let rows = conn
            .query(query, &[&class_code, &instrument_status])
            .await
            .map_err(|e| {
                error!(
                    "error executing the request to get a list of tools: {:?}",
                    e
                );
                e
            })?;

        // Print column names
        println!(
            "{:<12} | {:>32} | {:>12} | {:>20} | {:>30}",
            "class_code", "status", "sec_code", "instr_short_name", "update_timestamptz"
        );

        // Print the dividing line
        println!(
            "{:-<12}-+-{:-<32}-+-{:-<12}-+-{:-<20}-+-{:-<30}-",
            "", "", "", "", ""
        );

        // Creating a vector for Instruments
        let mut instruments: Vec<Instruments> = Vec::new();

        // Processing the results SQL request
        for row in rows {
            let class_code: String = row.get("class_code");
            let instrument_status: String = row.get("instrument_status");
            let sec_code: String = row.get("sec_code");
            let instr_short_name: String = row.get("instr_short_name");
            let update_timestamptz: DateTime<Local> = row.get("update_timestamptz");

            println!(
                "{:<12} | {:>32} | {:>12} | {:>20} | {:>30}",
                class_code, instrument_status, sec_code, instr_short_name, update_timestamptz
            );

            let hysteresis_percentage = 1.0; // %
            let hysteresis_periods = 5; // periods
            let crossover_signal = CrossoverSignal::new(hysteresis_percentage, hysteresis_periods);

            let instr = Instruments {
                // class_code,
                // instrument_status,
                sec_code,
                // instr_short_name,
                crossover_signal,
            };

            instruments.push(instr);
        }
        Ok(instruments)
    }

    /// Get trading data for calculating the EMA
    pub async fn get_data_for_ema(
        &self,
        sec_code: &str,
        interval: f64,
        period_len: f64,
    ) -> Result<Vec<DataForEma>, RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        let query = "
            WITH period_data AS (
                SELECT
                    ht.*,
                    NOW() - FLOOR(
                        EXTRACT(EPOCH FROM NOW() - ht.update_timestamptz) / $1::double precision
                    ) * $1::double precision * INTERVAL '1 second' AS period_start
                FROM
                    historical_trades ht
                WHERE
                    ht.sec_code = $3
                    AND ht.update_timestamptz >= NOW() - $2::double precision * INTERVAL '1 second'
            )
            SELECT
                period_start,
                (ARRAY_AGG(last_price ORDER BY update_timestamptz ASC))[1] AS open_price,
                (ARRAY_AGG(last_price ORDER BY update_timestamptz DESC))[1] AS close_price,
                MIN(last_price) AS min_price,
                MAX(last_price) AS max_price,
                SUM(last_volume) AS period_volume
            FROM
                period_data
            GROUP BY
                period_start
            ORDER BY
                period_start;
        ";

        // Executing a request with parameters
        let rows = conn
            .query(query, &[&period_len, &interval, &sec_code])
            .await
            .map_err(|e| {
                error!(
                    "error in executing the request to receive data for calculating the EMA: {:?}",
                    e
                );
                e
            })?;

        // Print column names
        println!(
            "{:<30} | {:>15} | {:>15} | {:>15} | {:>15} | {:>15}",
            "period_start", "open_price", "close_price", "min_price", "max_price", "period_volume"
        );

        // Print the dividing line
        println!(
            "{:-<30}-+-{:-<15}-+-{:-<15}-+-{:-<15}-+-{:-<15}-+-{:-<15}-",
            "", "", "", "", "", ""
        );

        // Create a vector for DataItem
        let mut data_item: Vec<DataForEma> = Vec::new();

        // Processing the results SQL request
        for row in rows {
            let period_start: DateTime<Local> = row.get("period_start");

            let open_price: f64 = row
                .try_get::<_, Decimal>("open_price")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let close_price: f64 = row
                .try_get::<_, Decimal>("close_price")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let min_price: f64 = row
                .try_get::<_, Decimal>("min_price")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let max_price: f64 = row
                .try_get::<_, Decimal>("max_price")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let period_volume: f64 = row
                .try_get::<_, Decimal>("period_volume")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            println!(
                "{:<30} | {:>15.6} | {:>15.6} | {:>15.6} | {:>15.6} | {:>15.6}",
                period_start, open_price, close_price, min_price, max_price, period_volume
            );

            let item = DataForEma {
                open: open_price,
                high: max_price,
                low: min_price,
                close: close_price,
                volume: period_volume,
            };

            data_item.push(item);
        }

        Ok(data_item)
    }
}
