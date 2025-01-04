//! # It works with exchange data from the PostgreSQL DBMS.
use crate::signal::CrossoverSignal;
use bb8::RunError;
use bb8_postgres::{bb8::Pool, tokio_postgres::NoTls, PostgresConnectionManager};
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use postgres_types::{FromSql, ToSql};
use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;
use tracing::error;

#[derive(Debug, ToSql, FromSql, PartialEq)]
#[postgres(name = "operation")]
pub enum Operation {
    #[postgres(name = "transaction_reply")]
    TransactionReply,
    #[postgres(name = "signal_buy")]
    SignalBuy,
    #[postgres(name = "signal_sell")]
    SignalSell,
    #[postgres(name = "order_buy")]
    OrderBuy,
    #[postgres(name = "order_sell")]
    OrderSell,
    #[postgres(name = "trade_buy")]
    TradeBuy,
    #[postgres(name = "trade_sell")]
    TradeSell,
    #[postgres(name = "none_operation")]
    NoneOperation,
}

#[derive(Debug)]
pub struct Ema {
    pub sec_code: String,
    pub short_ema: f64,
    pub long_ema: f64,
    pub last_price: f64,
    pub operation: Operation,
    pub timestamp: NaiveDateTime,
}

#[derive(Debug)]
pub struct Candle {
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl Candle {
    pub fn is_valid(&self) -> bool {
        // Define a small epsilon
        const EPSILON: f64 = 1e-10;

        let fields = [self.open, self.high, self.low, self.close, self.volume];

        fields
            .iter()
            .all(|&value| value.is_finite() && value > EPSILON)
    }
}

pub struct Db {
    pool: Pool<PostgresConnectionManager<NoTls>>,
}

/// Trading instruments
pub struct Instrument {
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
                trade_date DATE
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

    /// Create type operation
    pub async fn create_type_operation(
        &self,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        let query = "
                DO $$
                BEGIN
                    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'operation') THEN
                        CREATE TYPE operation AS ENUM (
                            'transaction_reply',
                            'signal_buy',
                            'signal_sell',
                            'order_buy',
                            'order_sell',
                            'trade_buy',
                            'trade_sell',
                            'none_operation'
                        );
                    END IF;
                END;
                $$;
            ";

        // Executing the command to create a type
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
                last_price DOUBLE PRECISION,
                operation OPERATION,
                update_timestamp TIMESTAMP DEFAULT NOW()
            );
        ";

        // Executing the command to create a table
        conn.execute(query, &[]).await?;

        Ok(())
    }

    /// Create type signal
    pub async fn create_type_signal(
        &self,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        let query = "
                DO $$
                BEGIN
                    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'signal') THEN
                        CREATE TYPE signal AS ENUM (
                            'buy',
                            'sell'
                        );
                    END IF;
                END;
                $$;
            ";

        // Executing the command to create a type
        conn.execute(query, &[]).await?;

        Ok(())
    }

        /// Create type state
        pub async fn create_type_state(
            &self,
        ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
            // Get a connection from the pool
            let conn = self.pool.get().await.map_err(|e| {
                error!("error get a connection from the pool: {:?}", e);
                e
            })?;
    
            let query = "
                    DO $$
                    BEGIN
                        IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'state') THEN
                            CREATE TYPE state AS ENUM (
                                'above',
                                'below',
                                'between'
                            );
                        END IF;
                    END;
                    $$;
                ";
    
            // Executing the command to create a type
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

        if let Err(e) = self.insert_into_historical().await {
            error!("create function insert_into_historical error: {}", e);
        }

        if let Err(e) = self.before_update_current_trades().await {
            error!("create trigger before_update_current_trades error: {}", e);
        }

        if let Err(e) = self.create_type_operation().await {
            error!("create type operation error: {}", e);
        }

        if let Err(e) = self.create_ema().await {
            error!("create table ema error: {}", e);
        }

        if let Err(e) = self.create_type_signal().await {
            error!("create type signal error: {}", e);
        }

        if let Err(e) = self.create_type_state().await {
            error!("create type signal error: {}", e);
        }

        Ok(())
    }

    pub async fn get_instruments(
        &self,
        class_code: &str,
        instrument_status: &str,
    ) -> Result<Vec<Instrument>, RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        let query = "
        SELECT DISTINCT ON (sec_code)
            class_code, instrument_status, sec_code, instr_short_name, trade_date, last_price_time
        FROM historical_trades
        WHERE class_code = $1
          AND instrument_status = $2
          AND trade_date IS NOT NULL
          AND last_price_time IS NOT NULL
        ORDER BY sec_code ASC, trade_date DESC, last_price_time DESC; 
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
            "{:>12} | {:>32} | {:>12} | {:>20} | {:>10} | {:>8}",
            "class_code",
            "instrument_status",
            "sec_code",
            "instr_short_name",
            "trade_date",
            "last_price_time"
        );

        // Print the dividing line
        println!(
            "{:-<12}-+-{:-<32}-+-{:-<12}-+-{:-<20}-+-{:-<10}-+-{:-<8}-",
            "", "", "", "", "", ""
        );

        // Creating a vector for Instruments
        let mut instruments: Vec<Instrument> = Vec::new();

        // Processing the results SQL request
        for row in rows {
            let class_code: String = row.get("class_code");
            let instrument_status: String = row.get("instrument_status");
            let sec_code: String = row.get("sec_code");
            let instr_short_name: String = row.get("instr_short_name");
            let trade_date: NaiveDate = row.get("trade_date");
            let last_price_time: NaiveTime = row.get("last_price_time");

            println!(
                "{:<12} | {:>32} | {:>12} | {:>20} | {:>10} | {:>8}",
                class_code,
                instrument_status,
                sec_code,
                instr_short_name,
                trade_date,
                last_price_time
            );

            let hysteresis_percentage = 2.0; // %
            let hysteresis_periods = 5; // periods
            let crossover_signal = CrossoverSignal::new(hysteresis_percentage, hysteresis_periods);

            let instrument = Instrument {
                sec_code,
                crossover_signal,
            };

            instruments.push(instrument);
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
            WITH
            params AS (
            SELECT
                $1::INT AS timeframe_min,   -- Timeframe in minutes
                $2::INT AS num_candles,     -- Number of candles to retrieve
                $3::TEXT AS sec_code        -- Security code
            ),
            -- Determine the current time in UTC+3
            current_ts AS (
            SELECT (NOW() AT TIME ZONE 'UTC') + INTERVAL '3 hours' AS ts
            ),
            -- Compute the timestamp of the last fully completed candle before current time
            last_candle_start_ts AS (
            SELECT
                timestamp 'epoch' + FLOOR(
                (EXTRACT(EPOCH FROM ts) - (60 * (SELECT timeframe_min FROM params))) / (60 * (SELECT timeframe_min FROM params))
                ) * (60 * (SELECT timeframe_min FROM params)) * INTERVAL '1 second' AS ts
            FROM
                current_ts
            ),
            candle_times AS (
            SELECT
                ct.candle_start,
                ct.candle_start + (SELECT timeframe_min FROM params) * INTERVAL '1 minute' AS candle_end
            FROM
                (
                SELECT
                    generate_series(
                    -- Start from an earlier candle to account for excluded candles
                    (SELECT ts FROM last_candle_start_ts) - ((SELECT num_candles FROM params) + 24 - 1) * (SELECT timeframe_min FROM params) * INTERVAL '1 minute',
                    -- End at the last fully completed candle
                    (SELECT ts FROM last_candle_start_ts),
                    -- Increment by the timeframe
                    (SELECT timeframe_min FROM params) * INTERVAL '1 minute'
                    ) AS candle_start
                ) ct
            WHERE
                -- Exclude candles during non-trading hours (updated schedule)
                NOT (
                ct.candle_start::time >= TIME '02:00:00' AND ct.candle_start::time < TIME '04:00:00'
                )
            ),
            candles AS (
            SELECT
                ct.candle_start,
                ct.candle_end,
                (ARRAY_AGG(ht.last_price ORDER BY ht.trade_date + ht.last_price_time))[1] AS o,
                MAX(ht.last_price) AS h,
                MIN(ht.last_price) AS l,
                (ARRAY_AGG(ht.last_price ORDER BY ht.trade_date + ht.last_price_time DESC))[1] AS c,
                SUM(ht.last_volume) AS v
            FROM
                candle_times ct
                LEFT JOIN historical_trades ht ON ht.sec_code = (SELECT sec_code FROM params)
                AND (ht.trade_date + ht.last_price_time) >= ct.candle_start
                AND (ht.trade_date + ht.last_price_time) < ct.candle_end
            GROUP BY
                ct.candle_start,
                ct.candle_end
            ORDER BY
                ct.candle_start DESC
            )
        SELECT
            candle_start,
            candle_end,
            COALESCE(o, LAG(c) OVER (ORDER BY candle_start DESC)) AS open,
            COALESCE(h, COALESCE(o, LAG(c) OVER (ORDER BY candle_start DESC))) AS high,
            COALESCE(l, COALESCE(o, LAG(c) OVER (ORDER BY candle_start DESC))) AS low,
            COALESCE(c, COALESCE(o, LAG(c) OVER (ORDER BY candle_start DESC))) AS close,
            COALESCE(v, 0) AS volume
        FROM
            candles
        ORDER BY
            candle_start DESC
        LIMIT
            (SELECT num_candles FROM params);                           
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
            "{:>19} | {:>19} | {:>15} | {:>15} | {:>15} | {:>15} | {:>15}",
            "candle_start", "candle_end", "open", "high", "low", "close", "volume"
        );

        // Print the dividing line
        println!(
            "{:-<19}-+-{:-<19}-+-{:-<15}-+-{:-<15}-+-{:-<15}-+-{:-<15}-+-{:-<15}-",
            "", "", "", "", "", "", ""
        );

        // Create a vector for DataItem
        let mut candles: Vec<Candle> = Vec::new();

        // Processing the results SQL request
        for row in rows {
            let candle_start: NaiveDateTime = row.get("candle_start");

            let candle_end: NaiveDateTime = row.get("candle_end");

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
                "{:>19} | {:>19} | {:>15.6} | {:>15.6} | {:>15.6} | {:>15.6} | {:>15.6}",
                candle_start, candle_end, open, high, low, close, volume
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

    /// Get last_price from historical_trades table
    pub async fn get_last_price(
        &self,
        sec_code: &str,
    ) -> Result<f64, RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        let query = "
            SELECT last_price
            FROM historical_trades
            WHERE sec_code = $1
            ORDER BY trade_date DESC, last_price_time DESC
            LIMIT 1;
        ";

        // Executing
        let row = conn.query_one(query, &[&sec_code]).await.map_err(|e| {
            error!("get last_price error: {}", e);
            e
        })?;

        let last_price: f64 = row
            .try_get::<_, Decimal>("last_price")
            .ok()
            .and_then(|dec| dec.to_f64())
            .unwrap();

        Ok(last_price)
    }

    /// Insert EMA in ema table
    pub async fn insert_ema(
        &self,
        sec_code: &str,
        short_ema: f64,
        long_ema: f64,
        last_price: f64,
        operation: Operation,
        update_timestamp: NaiveDateTime,
    ) -> Result<(), RunError<bb8_postgres::tokio_postgres::Error>> {
        // Get a connection from the pool
        let conn = self.pool.get().await.map_err(|e| {
            error!("error get a connection from the pool: {:?}", e);
            e
        })?;

        let query = "
            INSERT INTO ema (sec_code, short_ema, long_ema, last_price, operation, update_timestamp)
            VALUES ($1, $2, $3, $4, $5, $6);
        ";

        // Executing insert into ema
        conn.execute(
            query,
            &[
                &sec_code,
                &short_ema,
                &long_ema,
                &last_price,
                &operation,
                &update_timestamp,
            ],
        )
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
            SELECT sec_code, short_ema, long_ema, last_price, operation, update_timestamp
            FROM ema
            WHERE sec_code = $1
            ORDER BY update_timestamp DESC LIMIT 100000;
        ";

        // Executing
        let rows = conn.query(query, &[&sec_code]).await.map_err(|e| {
            error!("get ema error: {}", e);
            e
        })?;

        let mut ema: Vec<Ema> = Vec::new();

        for row in rows {
            let sec_code = row.get("sec_code");
            let short_ema = row.get("short_ema");
            let long_ema = row.get("long_ema");
            let last_price = row.get("last_price");
            let operation = row.get("operation");
            let timestamp: NaiveDateTime = row.get("update_timestamp");

            let ema_frame = Ema {
                sec_code,
                short_ema,
                long_ema,
                last_price,
                operation,
                timestamp,
            };

            ema.push(ema_frame);
        }

        Ok(ema)
    }
}
