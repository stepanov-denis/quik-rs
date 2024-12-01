//! # It works with exchange data from the PostgreSQL DBMS.
use crate::signal::CrossoverSignal;
use bb8::RunError;
use bb8_postgres::{bb8::Pool, tokio_postgres::NoTls, PostgresConnectionManager};
use chrono::{DateTime, Local, NaiveDateTime, Utc};
use eframe::glow::Query;
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use tracing::error;

#[derive(Debug)]
pub struct Ema {
    pub sec_code: String,
    pub short_ema: f64,
    pub long_ema: f64,
    pub timestamptz: DateTime<Utc>,
}

#[derive(Debug)]
pub struct Candle {
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

    /// Set default transaction isolation level for database
    /// It's worked for the next session
    pub async fn set_transaction_isolation(
        &self,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        // Set default transaction isolation level
        let query = "ALTER DATABASE postgres SET default_transaction_isolation TO 'serializable';";

        // Executing the command to set default transaction isolation level
        conn.execute(query, &[]).await?;

        Ok(())
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
                update_timestamptz TIMESTAMPTZ DEFAULT NOW() -- timestamp with time zone
            );
        ";

        // Executing the command to create a table
        conn.execute(query, &[]).await?;

        Ok(())
    }

    /// Creating a table of EMA
    pub async fn create_ema(&self) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        // Create table
        let query = "
            CREATE TABLE IF NOT EXISTS ema (
                id SERIAL PRIMARY KEY,
                sec_code VARCHAR(12),
                short_ema DOUBLE PRECISION,
                long_ema DOUBLE PRECISION,
                update_timestamptz TIMESTAMPTZ DEFAULT NOW() -- timestamp with time zone
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

    /// Initial database
    pub async fn init(&self) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        if let Err(e) = self.set_transaction_isolation().await {
            error!("set default transaction isolation level error: {}", e);
        }
        if let Err(e) = self.create_current_trades().await {
            error!("create table current_trades error: {}", e);
        }

        if let Err(e) = self.create_historical_trades().await {
            error!("create table historical_trades error: {}", e);
        }

        if let Err(e) = self.create_ema().await {
            error!("create table ema error: {}", e);
        }

        if let Err(e) = self.insert_into_historical().await {
            error!("create function insert_into_historical error: {}", e);
        }

        if let Err(e) = self.before_update_current_trades().await {
            error!("create trigger before_update_current_trades error: {}", e);
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
                sec_code,
                crossover_signal,
            };

            instruments.push(instr);
        }
        Ok(instruments)
    }

    /// Get trading data for calculating the EMA
    pub async fn get_candles(
        &self,
        sec_code: &str,
        timeframe: i32,
        number_of_candles: i32,
    ) -> Result<Vec<Candle>, RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        let query = "
            WITH intervals AS (
                SELECT
                    generate_series(
                        date_trunc('minute', now() AT TIME ZONE 'UTC') - ((($2::integer + 2) * $1::integer) || ' minutes')::interval,
                        date_trunc('minute', now() AT TIME ZONE 'UTC'),
                        ($1::integer || ' minutes')::interval
                    ) AS interval_start
            ),
            candle_data AS (
                SELECT
                    interval_start,
                    (SELECT last_price FROM historical_trades WHERE trade_date + last_price_time AT TIME ZONE 'UTC' >= interval_start AND trade_date + last_price_time AT TIME ZONE 'UTC' < interval_start + ($1::integer || ' minutes')::interval AND sec_code = $3 AND (last_price_time AT TIME ZONE 'UTC' BETWEEN '01:05:00' AND '23:00:00') ORDER BY trade_date + last_price_time ASC LIMIT 1) AS open,
                    (SELECT MAX(last_price) FROM historical_trades WHERE trade_date + last_price_time AT TIME ZONE 'UTC' >= interval_start AND trade_date + last_price_time AT TIME ZONE 'UTC' < interval_start + ($1::integer || ' minutes')::interval AND sec_code = $3 AND (last_price_time AT TIME ZONE 'UTC' BETWEEN '01:05:00' AND '23:00:00')) AS high,
                    (SELECT MIN(last_price) FROM historical_trades WHERE trade_date + last_price_time AT TIME ZONE 'UTC' >= interval_start AND trade_date + last_price_time AT TIME ZONE 'UTC' < interval_start + ($1::integer || ' minutes')::interval AND sec_code = $3 AND (last_price_time AT TIME ZONE 'UTC' BETWEEN '01:05:00' AND '23:00:00')) AS low,
                    (SELECT last_price FROM historical_trades WHERE trade_date + last_price_time AT TIME ZONE 'UTC' >= interval_start AND trade_date + last_price_time AT TIME ZONE 'UTC' < interval_start + ($1::integer || ' minutes')::interval AND sec_code = $3 AND (last_price_time AT TIME ZONE 'UTC' BETWEEN '01:05:00' AND '23:00:00') ORDER BY trade_date + last_price_time DESC LIMIT 1) AS close,
                    (SELECT SUM(last_volume) FROM historical_trades WHERE trade_date + last_price_time AT TIME ZONE 'UTC' >= interval_start AND trade_date + last_price_time AT TIME ZONE 'UTC' < interval_start + ($1::integer || ' minutes')::interval AND sec_code = $3 AND (last_price_time AT TIME ZONE 'UTC' BETWEEN '01:05:00' AND '23:00:00')) AS volume
                FROM intervals
                WHERE
                    interval_start::time BETWEEN '01:05:00' AND '23:00:00'
            ),
            final_candles AS (
                SELECT interval_start, open, high, low, close, volume
                FROM candle_data
                -- WHERE open IS NOT NULL
                ORDER BY interval_start DESC
                LIMIT $2::integer
            )
            SELECT *
            FROM final_candles
            ORDER BY interval_start;
        ";

        // Executing a request with parameters
        let rows = conn
            .query(query, &[&timeframe, &number_of_candles, &sec_code])
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
            "interval_start", "open", "high", "low", "close", "volume"
        );

        // Print the dividing line
        println!(
            "{:-<30}-+-{:-<15}-+-{:-<15}-+-{:-<15}-+-{:-<15}-+-{:-<15}-",
            "", "", "", "", "", ""
        );

        // Create a vector for DataItem
        let mut candles: Vec<Candle> = Vec::new();

        // Processing the results SQL request
        for row in rows {
            let interval_start: NaiveDateTime = row.get("interval_start");

            let open: f64 = row
                .try_get::<_, Decimal>("open")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let high: f64 = row
                .try_get::<_, Decimal>("high")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let low: f64 = row
                .try_get::<_, Decimal>("low")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let close: f64 = row
                .try_get::<_, Decimal>("close")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            let volume: f64 = row
                .try_get::<_, Decimal>("volume")
                .ok()
                .and_then(|dec| dec.to_f64())
                .unwrap_or_default();

            println!(
                "{:<30} | {:>15.6} | {:>15.6} | {:>15.6} | {:>15.6} | {:>15.6}",
                interval_start, open, high, low, close, volume
            );

            let candle = Candle {
                open,
                high,
                low,
                close,
                volume,
            };

            candles.push(candle);
        }

        Ok(candles)
    }

    /// Insert EMA in ema table
    pub async fn insert_ema(
        &self,
        sec_code: &str,
        short_ema: f64,
        long_ema: f64,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        let query = "
            INSERT INTO ema (sec_code, short_ema, long_ema)
            VALUES ($1, $2, $3);
        ";

        // Executing insert into ema
        conn.execute(query, &[&sec_code, &short_ema, &long_ema])
            .await?;

        Ok(())
    }

    /// Get EMA from ema table
    pub async fn get_ema(
        &self,
        sec_code: &str,
    ) -> Result<Vec<Ema>, RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        let query = "
            SELECT short_ema, long_ema, update_timestamptz
            FROM ema
            WHERE sec_code = $1
            ORDER BY update_timestamptz DESC LIMIT 1000;
        ";

        // Executing insert into ema
        let rows = conn.query(query, &[&sec_code]).await.map_err(|e| {
            error!("get ema error: {}", e);
            e
        })?;

        let mut ema: Vec<Ema> = Vec::new();

        for row in rows {
            let sec_code = String::from(sec_code);
            let short_ema = row.get("short_ema");
            let long_ema = row.get("long_ema");
            let timestamptz: DateTime<Utc> = row.get("update_timestamptz");

            let ema_frame = Ema {
                sec_code,
                short_ema,
                long_ema,
                timestamptz,
            };

            ema.push(ema_frame);
        }

        Ok(ema)
    }
}
