//! # Importing transactions, entering orders into the QUIK ARQA Technologies trading system via the API.
//!
//! This functionality is designed to send transactions,
//! the functionality is implemented through the API in the form of a library Trans2QUIK.dll .
//!
//! The library contains functions, when calling these functions, you can:
//! * Establish or break the connection between the QUIK Workplace and the library
//!   Trans2QUIK.dll
//! * Check if there is a connection between the QUIK Workplace and the library
//!   Trans2QUIK.dll and between the QUIK Workplace and the QUIK server.
//! * Send the transaction.
//! * Get information on applications and transactions.
//!
//! There are two ways to transfer transactions – synchronous and asynchronous, which
//! are implemented by separate functions:
//! * With synchronous transaction transfer, the function is exited only after
//!   receiving a response from the QUIK server. Therefore, synchronous transactions
//!   can only be sent sequentially, waiting for a response about each sent transaction –
//!   this method is simpler and more suitable for programmers with little
//!   software development experience.
//! * With asynchronous transaction transfer, the function is exited immediately.
//!   The callback function is used to receive a response about sent asynchronous transactions.
//!   The function is called every time a response
//!   is received about an executed or rejected transaction.
//!
//! A callback function is also provided to monitor connections between
//! the QUIK terminal and the library Trans2QUIK.dll and between the QUIK Workplace
//! and the QUIK server.
//!
//! To receive information about orders and transactions, the user must first
//! create a list of received instruments, separately for applications and transactions. Then
//! the procedure for obtaining information using the callback functions is started.
//! Upon termination of receiving information on applications and transactions, the lists
//! of received instruments are cleared.
#![allow(dead_code)]
use encoding_rs::WINDOWS_1251;
use lazy_static::lazy_static;
use libc::{c_char, c_double, c_long, c_ulonglong, intptr_t};
use libloading::{Error as LibloadingError, Library, Symbol};
use std::error;
use std::ffi::{CStr, CString, NulError};
use std::fmt;
use std::str;
use std::string::FromUtf8Error;
use std::sync::{Arc, Condvar, Mutex};
use tracing::{error, info};

lazy_static! {
    static ref ORDER_CALLBACK_RECEIVED: Arc<(Mutex<bool>, Condvar)> =
        Arc::new((Mutex::new(false), Condvar::new()));
    static ref TRADE_CALLBACK_RECEIVED: Arc<(Mutex<bool>, Condvar)> =
        Arc::new((Mutex::new(false), Condvar::new()));
    static ref TRANSACTION_CALLBACK_RECEIVED: Arc<(Mutex<bool>, Condvar)> =
        Arc::new((Mutex::new(false), Condvar::new()));
}

/// Prototype of a callback function for monitoring the connection status.
/// This function is used to track the state of the connection between the
/// Trans2QUIK.dll library and the QUIK terminal, as well as the connection
/// between the QUIK terminal and the server.
type Trans2QuikConnectionStatusCallback =
    unsafe extern "C" fn(connection_event: c_long, error_code: c_long, error_message: *mut c_char);

/// A prototype of the callback function for processing the received transaction information.
/// Attention! The submission of asynchronous transactions using
/// the callback function and synchronous transactions at the same time is prohibited.
/// This is due to the fact that it is impossible to correctly call
/// the callback function at a time when the synchronous transaction processing function has
/// not finished its work yet.
type Trans2QuikTransactionReplyCallback = unsafe extern "C" fn(
    result_code: c_long,
    error_code: c_long,
    reply_code: c_long,
    trans_id: c_long,
    order_num: c_ulonglong,
    error_message_ptr: *mut c_char,
    trans_reply_descriptor_ptr: *mut intptr_t,
);

/// A prototype of the callback function to get information about the order parameters.
type Trans2QuikOrderStatusCallback = unsafe extern "C" fn(
    mode: c_long,
    trans_id: c_long,
    order_num: c_ulonglong,
    class_code: *mut c_char,
    sec_code: *mut c_char,
    price: c_double,
    balance: i64,
    value: c_double,
    is_sell: c_long,
    status: c_long,
    order_descriptor: intptr_t,
);

/// A prototype of the callback function to get information about the trade.
type Trans2QuikTradeStatusCallback = unsafe extern "C" fn(
    mode: c_long,
    trade_num: c_ulonglong,
    order_num: c_ulonglong,
    class_code: *mut c_char,
    sec_code: *mut c_char,
    price: c_double,
    quantity: i64,
    is_sell: c_long,
    value: c_double,
    trade_descriptor: intptr_t,
);

/// Represents the state of order receipt.
#[derive(Debug, PartialEq)]
pub enum Mode {
    NewOrder = 0,
    InitialOrder = 1,
    LastOrderReceived = 2,
    Unknown,
}

impl From<c_long> for Mode {
    fn from(code: c_long) -> Self {
        match code {
            0 => Mode::NewOrder,
            1 => Mode::InitialOrder,
            2 => Mode::LastOrderReceived,
            _ => Mode::Unknown,
        }
    }
}

/// The TransID of the transaction that generated the request.
/// It has a value of `0` if the request was not generated by a transaction from a file,
/// or if the TransID is unknown.
#[derive(Debug, PartialEq)]
pub enum TransId {
    Id(c_long),
    Unknown(c_long),
}

impl From<c_long> for TransId {
    fn from(id: c_long) -> Self {
        match id {
            0 => TransId::Unknown(id),
            _ => TransId::Id(id),
        }
    }
}

/// Sending an application.
#[derive(Debug, PartialEq)]
pub enum IsSell {
    Buy = 0,
    Sell,
}

impl From<c_long> for IsSell {
    fn from(code: c_long) -> Self {
        match code {
            0 => IsSell::Buy,
            _ => IsSell::Sell,
        }
    }
}

/// Represents the execution status of an order.
#[derive(Debug, PartialEq)]
pub enum Status {
    Active = 1,
    Canceled = 2,
    Executed,
}

impl From<c_long> for Status {
    fn from(code: c_long) -> Self {
        match code {
            1 => Status::Active,
            2 => Status::Canceled,
            _ => Status::Executed,
        }
    }
}

/// Corresponds to the description of constants whose values are returned when exiting functions
/// and procedures in the library Trans2QUIK.dll:
/// ```
/// TRANS2QUIK_SUCCESS 0
/// TRANS2QUIK_FAILED 1
/// TRANS2QUIK_QUIK_TERMINAL_NOT_FOUND 2
/// TRANS2QUIK_DLL_VERSION_NOT_SUPPORTED 3
/// TRANS2QUIK_ALREADY_CONNECTED_TO_QUIK 4
/// TRANS2QUIK_WRONG_SYNTAX 5
/// TRANS2QUIK_QUIK_NOT_CONNECTED 6
/// TRANS2QUIK_DLL_NOT_CONNECTED 7
/// TRANS2QUIK_QUIK_CONNECTED 8
/// TRANS2QUIK_QUIK_DISCONNECTED 9
/// TRANS2QUIK_DLL_CONNECTED 10
/// TRANS2QUIK_DLL_DISCONNECTED 11
/// TRANS2QUIK_MEMORY_ALLOCATION_ERROR 12
/// TRANS2QUIK_WRONG_CONNECTION_HANDLE 13
/// TRANS2QUIK_WRONG_INPUT_PARAMS 14
/// ```
#[derive(Debug, PartialEq)]
#[repr(i32)]
pub enum Trans2QuikResult {
    Success = 0,
    Failed = 1,
    TerminalNotFound = 2,
    DllVersionNotSupported = 3,
    AlreadyConnectedToQuik = 4,
    WrongSyntax = 5,
    QuikNotConnected = 6,
    DllNotConnected = 7,
    QuikConnected = 8,
    QuikDisconnected = 9,
    DllConnected = 10,
    DllDisconnected = 11,
    MemoryAllocationError = 12,
    WrongConnectionHandle = 13,
    WrongInputParams = 14,
    Unknown,
}

impl From<c_long> for Trans2QuikResult {
    fn from(code: c_long) -> Self {
        match code {
            0 => Trans2QuikResult::Success,
            1 => Trans2QuikResult::Failed,
            2 => Trans2QuikResult::TerminalNotFound,
            3 => Trans2QuikResult::DllVersionNotSupported,
            4 => Trans2QuikResult::AlreadyConnectedToQuik,
            5 => Trans2QuikResult::WrongSyntax,
            6 => Trans2QuikResult::QuikNotConnected,
            7 => Trans2QuikResult::DllNotConnected,
            8 => Trans2QuikResult::QuikConnected,
            9 => Trans2QuikResult::QuikDisconnected,
            10 => Trans2QuikResult::DllConnected,
            11 => Trans2QuikResult::DllDisconnected,
            12 => Trans2QuikResult::MemoryAllocationError,
            13 => Trans2QuikResult::WrongConnectionHandle,
            14 => Trans2QuikResult::WrongInputParams,
            _ => Trans2QuikResult::Unknown,
        }
    }
}

/// Сomposite error type for calling functions from the library Trans2QUIK.dll.
#[derive(Debug)]
pub enum Trans2QuikError {
    LibLoading(LibloadingError),
    NulError(NulError),
}

impl fmt::Display for Trans2QuikError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl error::Error for Trans2QuikError {}

impl From<LibloadingError> for Trans2QuikError {
    fn from(err: LibloadingError) -> Trans2QuikError {
        Trans2QuikError::LibLoading(err)
    }
}

impl From<NulError> for Trans2QuikError {
    fn from(err: NulError) -> Trans2QuikError {
        Trans2QuikError::NulError(err)
    }
}

/// The `Terminal` structure is used to interact with the QUIK trading terminal through the library Trans2QUIK.dll.
///
/// This structure provides loading of the DLL library Trans2QUIK.dll, establishing a connection to the QUIK terminal
/// and calling functions from the library to control the terminal and perform trading operations.
///
/// # Example of use
/// Cargo.toml
/// ```
/// trans2quik = "1.0.0"
/// tracing = "0.1.40"
/// tracing-subscriber = "0.3.18"
/// lazy_static = "1.5.0"
/// ```
/// main.rs
/// ```
/// use tracing::info;
/// use tracing_subscriber;
/// use lazy_static::lazy_static;
/// use std::sync::{Arc, Mutex, Condvar};
/// use std::time::Duration;
/// use std::error::Error;
/// use trans2quik;
///
/// lazy_static! {
///     static ref ORDER_CALLBACK_RECEIVED: Arc<(Mutex<bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));
///     static ref TRADE_CALLBACK_RECEIVED: Arc<(Mutex<bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));
///     static ref TRANSACTION_CALLBACK_RECEIVED: Arc<(Mutex<bool>, Condvar)> = Arc::new((Mutex::new(false), Condvar::new()));
/// }
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     tracing_subscriber::fmt::init();
///
///     let path = r"c:\QUIK Junior\trans2quik.dll";
///     let terminal = trans2quik::Terminal::new(path)?;
///     terminal.connect()?;
///     terminal.is_dll_connected()?;
///     terminal.is_quik_connected()?;
///     terminal.set_connection_status_callback()?;
///     terminal.set_transactions_reply_callback()?;
///     let class_code = "QJSIM";
///     let sec_code = "LKOH";
///     terminal.subscribe_orders(class_code, sec_code)?;
///     terminal.subscribe_trades(class_code, sec_code)?;
///     terminal.start_orders();
///     terminal.start_trades();
///     let transaction_str = "ACCOUNT=NL0011100043; CLIENT_CODE=10677; TYPE=L; TRANS_ID=1; CLASSCODE=QJSIM; SECCODE=LKOH; ACTION=NEW_ORDER; OPERATION=B; PRICE=7103,5; QUANTITY=1;";
///     terminal.send_async_transaction(transaction_str)?;
///
///     // Waiting for callback or timeout
///     {
///         let order_received = {
///             let (lock, cvar) = ORDER_CALLBACK_RECEIVED.as_ref();
///             let received = lock.lock().unwrap();
///             let timeout = Duration::from_secs(10);
///
///             let (received, timeout_result) = cvar
///                 .wait_timeout_while(received, timeout, |received| !*received)
///                 .unwrap();
///   
///             if timeout_result.timed_out() {
///                 info!("Timed out waiting for order_status_callback");
///             }
///
///             *received
///         };
///
///         let trade_received = {
///             let (lock, cvar) = TRADE_CALLBACK_RECEIVED.as_ref();
///             let received = lock.lock().unwrap();
///             let timeout = Duration::from_secs(10);
///
///             let (received, timeout_result) = cvar
///                 .wait_timeout_while(received, timeout, |received| !*received)
///                 .unwrap();
///
///             if timeout_result.timed_out() {
///                 info!("Timed out waiting for trade_status_callback");
///             }
///
///             *received
///         };
///
///         let transaction_received = {
///             let (lock, cvar) = TRANSACTION_CALLBACK_RECEIVED.as_ref();
///             let received = lock.lock().unwrap();
///             let timeout = Duration::from_secs(10);
///
///             let (received, timeout_result) = cvar
///                 .wait_timeout_while(received, timeout, |received| !*received)
///                 .unwrap();
///
///             if timeout_result.timed_out() {
///                 info!("Timed out waiting for transaction_reply_callback");
///             }
///
///             *received
///         };
///
///         if !order_received && !trade_received && !transaction_received {
///             info!("Did not receive all expected callbacks");
///         }
///     }    
///
///     terminal.unsubscribe_orders()?;
///     terminal.unsubscribe_trades()?;
///     terminal.disconnect()?;
///
///     Ok(())
/// }
/// ```
pub struct Terminal {
    /// Loading a dynamic library Trans2QUIK.dll, which provides an API for interacting with QUIK.
    library: Library,

    /// Calling a function from the library Trans2QUIK.dll for establishing communication with the QUIK terminal.
    trans2quik_connect:
        unsafe extern "C" fn(*mut c_char, *mut c_long, *mut c_char, c_long) -> c_long,

    /// Calling a function from the library Trans2QUIK.dll to disconnecting from the QUIK terminal.
    trans2quik_disconnect: unsafe extern "C" fn(*mut c_long, *mut c_char, c_long) -> c_long,

    /// Calling a function from the library Trans2QUIK.dll to check for a connection between the QUIK terminal and the server.
    trans2quik_is_quik_connected: unsafe extern "C" fn(*mut c_long, *mut c_char, c_long) -> c_long,

    /// Calling a function from the library Trans2QUIK.dll to check if there is a connection between the library Trans2QUIK.dll and the QUIK terminal.
    trans2quik_is_dll_connected: unsafe extern "C" fn(*mut c_long, *mut c_char, c_long) -> c_long,

    /// Sending a transaction synchronously. When sending synchronously, the return from the function occurs
    /// only after receiving the result of the transaction, or after disconnecting the
    /// QUIK terminal from the server.
    trans2quik_send_sync_transaction: unsafe extern "C" fn(
        trans_str_ptr: *mut c_char,
        reply_code_ptr: *mut c_long,
        trans_id_ptr: *mut c_long,
        order_num_ptr: *mut c_double,
        result_message_ptr: *mut c_char,
        result_message_len: c_long,
        error_code_ptr: *mut c_long,
        error_message_ptr: *mut c_char,
        error_message_len: c_long,
    ) -> c_long,

    /// Asynchronous transfer of a transaction. When sending an asynchronous transaction, the refund is
    /// the function is executed immediately, and the result of the transaction is reported via
    /// the corresponding callback function.
    trans2quik_send_async_transaction:
        unsafe extern "C" fn(*mut c_char, *mut c_long, *mut c_char, c_long) -> c_long,

    /// А callback function for processing the received connection information.
    trans2quik_set_connection_status_callback: unsafe extern "C" fn(
        Trans2QuikConnectionStatusCallback,
        *mut c_long,
        *mut c_char,
        c_long,
    ) -> c_long,

    /// Sets the callback function to receive information about the sent asynchronous transaction.
    trans2quik_set_transactions_reply_callback: unsafe extern "C" fn(
        Trans2QuikTransactionReplyCallback,
        *mut c_long,
        *mut c_char,
        c_long,
    ) -> c_long,

    /// The function is used to create a list of classes and tools for subscribing to receive orders for them.
    trans2quik_subscribe_orders:
        unsafe extern "C" fn(class_code: *mut c_char, sec_code: *mut c_char) -> c_long,

    /// The function is used to create a list of classes and tools for subscribing to receive trades on them.
    trans2quik_subscribe_trades:
        unsafe extern "C" fn(class_code: *mut c_char, sec_code: *mut c_char) -> c_long,

    /// The function starts the process of receiving requests for classes and tools defined
    /// by the TRANS2QUIK_SUBSCRIBE_ORDERS function.
    trans2quik_start_orders: unsafe extern "C" fn(Trans2QuikOrderStatusCallback),

    /// The function starts the process of receiving transactions with the parameters set
    /// by the function TRANS2QUIK_SUBSCRIBE_TRADES.
    trans2quik_start_trades: unsafe extern "C" fn(Trans2QuikTradeStatusCallback),

    /// The function interrupts the operation of the TRANS2QUIK_START_ORDERS function and clears
    /// the list of received tools generated by the function
    /// TRANS2QUIK_SUBSCRIBE_ORDERS.
    trans2quik_unsubscribe_orders: unsafe extern "C" fn() -> c_long,

    /// The function interrupts the operation of the TRANS2QUIK_START_TRADES function and clears
    /// the list of received tools generated by the function
    /// TRANS2QUIK_SUBSCRIBE_TRADES.
    trans2quik_unsubscribe_trades: unsafe extern "C" fn() -> c_long,
}

impl Terminal {
    /// The function is used to load the library Trans2QUIK.dll.
    pub fn new(path: &str) -> Result<Self, Trans2QuikError> {
        // Loading a dynamic library Trans2QUIK.dll, which provides an API for interacting with QUIK.
        let library = unsafe { Library::new(path)? };

        // Calling a function from the library Trans2QUIK.dll for establishing communication with the QUIK terminal.
        let trans2quik_connect = load_symbol::<
            unsafe extern "C" fn(*mut c_char, *mut c_long, *mut c_char, c_long) -> c_long,
        >(&library, b"TRANS2QUIK_CONNECT\0")?;

        // Calling a function from the library Trans2QUIK.dll to disconnecting from the QUIK terminal.
        let trans2quik_disconnect = load_symbol::<
            unsafe extern "C" fn(*mut c_long, *mut c_char, c_long) -> c_long,
        >(&library, b"TRANS2QUIK_DISCONNECT\0")?;

        // Calling a function from the library Trans2QUIK.dll to check for a connection between the QUIK terminal and the server.
        let trans2quik_is_quik_connected = load_symbol::<
            unsafe extern "C" fn(*mut c_long, *mut c_char, c_long) -> c_long,
        >(&library, b"TRANS2QUIK_IS_QUIK_CONNECTED\0")?;

        // Calling a function from the library Trans2QUIK.dll to check if there is a connection between the library Trans2QUIK.dll and the QUIK terminal.
        let trans2quik_is_dll_connected = load_symbol::<
            unsafe extern "C" fn(*mut c_long, *mut c_char, c_long) -> c_long,
        >(&library, b"TRANS2QUIK_IS_DLL_CONNECTED\0")?;

        // Sending a transaction synchronously. When sending synchronously, the return from the function occurs
        // only after receiving the result of the transaction, or after disconnecting the
        // QUIK terminal from the server.
        let trans2quik_send_sync_transaction =
            load_symbol::<
                unsafe extern "C" fn(
                    *mut c_char,
                    *mut c_long,
                    *mut c_long,
                    *mut c_double,
                    *mut c_char,
                    c_long,
                    *mut c_long,
                    *mut c_char,
                    c_long,
                ) -> c_long,
            >(&library, b"TRANS2QUIK_SEND_SYNC_TRANSACTION\0")?;

        // Asynchronous transfer of a transaction. When sending an asynchronous transaction, the refund is
        // the function is executed immediately, and the result of the transaction is reported via
        // the corresponding callback function.
        let trans2quik_send_async_transaction =
            load_symbol::<
                unsafe extern "C" fn(*mut c_char, *mut c_long, *mut c_char, c_long) -> c_long,
            >(&library, b"TRANS2QUIK_SEND_ASYNC_TRANSACTION\0")?;

        // А callback function for processing the received connection information.
        let trans2quik_set_connection_status_callback =
            load_symbol::<
                unsafe extern "C" fn(
                    Trans2QuikConnectionStatusCallback,
                    *mut c_long,
                    *mut c_char,
                    c_long,
                ) -> c_long,
            >(&library, b"TRANS2QUIK_SET_CONNECTION_STATUS_CALLBACK\0")?;

        // Sets the callback function to receive information about the sent asynchronous transaction.
        let trans2quik_set_transactions_reply_callback =
            load_symbol::<
                unsafe extern "C" fn(
                    Trans2QuikTransactionReplyCallback,
                    *mut c_long,
                    *mut c_char,
                    c_long,
                ) -> c_long,
            >(&library, b"TRANS2QUIK_SET_TRANSACTIONS_REPLY_CALLBACK\0")?;

        // The function is used to create a list of classes and tools for subscribing to receive
        // applications for them.
        let trans2quik_subscribe_orders = load_symbol::<
            unsafe extern "C" fn(*mut c_char, *mut c_char) -> c_long,
        >(&library, b"TRANS2QUIK_SUBSCRIBE_ORDERS\0")?;

        // The function is used to create a list of classes and tools for subscribing to receive deals on them.
        let trans2quik_subscribe_trades = load_symbol::<
            unsafe extern "C" fn(*mut c_char, *mut c_char) -> c_long,
        >(&library, b"TRANS2QUIK_SUBSCRIBE_TRADES\0")?;

        // The function starts the process of receiving requests for classes and tools defined
        // by the TRANS2QUIK_SUBSCRIBE_ORDERS function.
        let trans2quik_start_orders = load_symbol::<
            unsafe extern "C" fn(Trans2QuikOrderStatusCallback),
        >(&library, b"TRANS2QUIK_START_ORDERS\0")?;

        // The function starts the process of receiving transactions with the parameters set
        // by the function TRANS2QUIK_SUBSCRIBE_TRADES.
        let trans2quik_start_trades = load_symbol::<
            unsafe extern "C" fn(Trans2QuikTradeStatusCallback),
        >(&library, b"TRANS2QUIK_START_TRADES\0")?;

        // The function interrupts the operation of the TRANS2QUIK_START_ORDERS function and clears
        // the list of received tools generated by the function
        // TRANS2QUIK_SUBSCRIBE_ORDERS.
        let trans2quik_unsubscribe_orders = load_symbol::<unsafe extern "C" fn() -> c_long>(
            &library,
            b"TRANS2QUIK_UNSUBSCRIBE_ORDERS\0",
        )?;

        // The function interrupts the operation of the TRANS2QUIK_START_TRADES function and clears
        // the list of received tools generated by the function
        // TRANS2QUIK_SUBSCRIBE_TRADES.
        let trans2quik_unsubscribe_trades = load_symbol::<unsafe extern "C" fn() -> c_long>(
            &library,
            b"TRANS2QUIK_UNSUBSCRIBE_TRADES\0",
        )?;

        Ok(Terminal {
            library,
            trans2quik_connect,
            trans2quik_disconnect,
            trans2quik_is_quik_connected,
            trans2quik_is_dll_connected,
            trans2quik_send_sync_transaction,
            trans2quik_send_async_transaction,
            trans2quik_set_connection_status_callback,
            trans2quik_set_transactions_reply_callback,
            trans2quik_subscribe_orders,
            trans2quik_subscribe_trades,
            trans2quik_start_orders,
            trans2quik_start_trades,
            trans2quik_unsubscribe_orders,
            trans2quik_unsubscribe_trades,
        })
    }

    /// Calling a function from the library Trans2QUIK.dll.
    fn call_trans2quik_function<F>(
        &self,
        function_name: &str,
        func: F,
    ) -> Result<Trans2QuikResult, Trans2QuikError>
    where
        F: FnOnce(*mut c_long, *mut c_char, c_long) -> c_long,
    {
        let mut error_code: c_long = 0;
        let error_code_ptr = &mut error_code as *mut c_long;

        let mut error_message = vec![0 as c_char; 256];
        let error_message_ptr = error_message.as_mut_ptr() as *mut c_char;

        // Вызов функции
        let function_result = func(
            error_code_ptr,
            error_message_ptr,
            error_message.len() as c_long,
        );

        let error_message = match extract_string_from_vec(error_message) {
            Ok(message) => message,
            Err(e) => {
                error!("Warning: error_message contains invalid UTF-8: {}", e);
                String::from("Invalid UTF-8 in error_message")
            }
        };

        let trans2quik_result = Trans2QuikResult::from(function_result);

        info!(
            "{} -> {:?}, error_code: {}, error_message: {}",
            function_name, trans2quik_result, error_code, error_message
        );
        Ok(trans2quik_result)
    }

    /// The function is used to establish communication with the QUIK terminal.
    pub fn connect(&self) -> Result<Trans2QuikResult, Trans2QuikError> {
        let connection_str = CString::new(r"c:\QUIK Junior")?;
        let connection_str_ptr = connection_str.as_ptr() as *mut c_char;

        let function = |error_code_ptr: *mut c_long,
                        error_message_ptr: *mut c_char,
                        error_message_len: c_long| unsafe {
            (self.trans2quik_connect)(
                connection_str_ptr,
                error_code_ptr,
                error_message_ptr,
                error_message_len,
            )
        };

        self.call_trans2quik_function("TRANS2QUIK_CONNECT", function)
    }

    /// The function is used to disconnect from the QUIK terminal.
    pub fn disconnect(&self) -> Result<Trans2QuikResult, Trans2QuikError> {
        let function = |error_code: *mut c_long,
                        error_message: *mut c_char,
                        error_message_len: c_long| unsafe {
            (self.trans2quik_disconnect)(error_code, error_message, error_message_len)
        };

        self.call_trans2quik_function("TRANS2QUIK_DISCONNECT", function)
    }

    /// The function is used to check if there is a connection between the QUIK terminal and the server.
    pub fn is_quik_connected(&self) -> Result<Trans2QuikResult, Trans2QuikError> {
        let function = |error_code: *mut c_long,
                        error_message: *mut c_char,
                        error_message_len: c_long| unsafe {
            (self.trans2quik_is_quik_connected)(error_code, error_message, error_message_len)
        };

        self.call_trans2quik_function("TRANS2QUIK_IS_QUIK_CONNECTED", function)
    }

    /// Checking for a connection between the library Trans2QUIK.dll and the QUIK terminal.
    pub fn is_dll_connected(&self) -> Result<Trans2QuikResult, Trans2QuikError> {
        let function = |error_code: *mut c_long,
                        error_message: *mut c_char,
                        error_message_len: c_long| unsafe {
            (self.trans2quik_is_dll_connected)(error_code, error_message, error_message_len)
        };

        self.call_trans2quik_function("TRANS2QUIK_IS_DLL_CONNECTED", function)
    }

    /// Sending a transaction synchronously. When sending synchronously, the return from the function occurs
    /// only after receiving the result of the transaction, or after disconnecting the
    /// QUIK terminal from the server.
    pub fn send_sync_transaction(
        &self,
        transaction_str: &str,
    ) -> Result<Trans2QuikResult, Trans2QuikError> {
        let trans_str = CString::new(transaction_str)?;
        let trans_str_ptr = trans_str.as_ptr() as *mut c_char;

        let mut reply_code: c_long = 0;
        let reply_code_ptr = &mut reply_code as *mut c_long;

        let mut trans_id: c_long = 0;
        let trans_id_ptr = &mut trans_id as *mut c_long;

        let mut order_num: c_double = 0.0;
        let order_num_ptr = &mut order_num as *mut c_double;

        let mut result_message = vec![0 as c_char; 256];
        let result_message_ptr = result_message.as_mut_ptr() as *mut c_char;

        let mut error_code: c_long = 0;
        let error_code_ptr = &mut error_code as *mut c_long;

        let mut error_message = vec![0 as c_char; 256];
        let error_message_ptr = error_message.as_mut_ptr() as *mut c_char;

        let function_result = unsafe {
            (self.trans2quik_send_sync_transaction)(
                trans_str_ptr,
                reply_code_ptr,
                trans_id_ptr,
                order_num_ptr,
                result_message_ptr,
                result_message.len() as c_long,
                error_code_ptr,
                error_message_ptr,
                error_message.len() as c_long,
            )
        };

        let result_message = match extract_string_from_vec(result_message) {
            Ok(message) => message,
            Err(e) => {
                error!("Warning: result_message contains invalid UTF-8: {}", e);
                String::from("Invalid UTF-8 in result_message")
            }
        };

        let error_message = match extract_string_from_vec(error_message) {
            Ok(message) => message,
            Err(e) => {
                error!("Warning: error_message contains invalid UTF-8: {}", e);
                String::from("Invalid UTF-8 in error_message")
            }
        };

        let trans2quik_result = Trans2QuikResult::from(function_result);

        info!("TRANS2QUIK_SEND_SYNC_TRANSACTION -> {:?}, reply_code: {}, trans_id: {}, order_num: {}, result_message: {}, error_code: {}, error_message: {}",
            trans2quik_result,
            reply_code,
            trans_id,
            order_num,
            result_message,
            error_code,
            error_message,
        );

        Ok(trans2quik_result)
    }

    /// Asynchronous transfer of a transaction. When sending an asynchronous transaction, the refund is
    /// the function is executed immediately, and the result of the transaction is reported via
    /// the corresponding callback function.
    pub fn send_async_transaction(
        &self,
        transaction_str: &str,
    ) -> Result<Trans2QuikResult, Trans2QuikError> {
        let trans_str = CString::new(transaction_str)?;
        let trans_str_ptr = trans_str.as_ptr() as *mut c_char;

        let mut error_code: c_long = 0;
        let error_code_ptr = &mut error_code as *mut c_long;

        let mut error_message = vec![0 as c_char; 256];
        let error_message_ptr = error_message.as_mut_ptr() as *mut c_char;

        let function_result = unsafe {
            (self.trans2quik_send_async_transaction)(
                trans_str_ptr,
                error_code_ptr,
                error_message_ptr,
                error_message.len() as c_long,
            )
        };

        let error_message = match extract_string_from_vec(error_message) {
            Ok(message) => message,
            Err(e) => {
                error!("Warning: error_message contains invalid UTF-8: {}", e);
                String::from("Invalid UTF-8 in error_message")
            }
        };

        let trans2quik_result = Trans2QuikResult::from(function_result);

        info!(
            "TRANS2QUIK_SEND_ASYNC_TRANSACTION -> {:?}, error_code: {}, error_message: {}",
            trans2quik_result, error_code, error_message,
        );

        Ok(trans2quik_result)
    }

    /// А callback function for processing the received connection information.
    pub fn set_connection_status_callback(&self) -> Result<Trans2QuikResult, Trans2QuikError> {
        let mut error_code: c_long = 0;
        let error_code_ptr = &mut error_code as *mut c_long;

        let mut error_message = vec![0 as c_char; 256];
        let error_message_ptr = error_message.as_mut_ptr() as *mut c_char;

        let function_result = unsafe {
            (self.trans2quik_set_connection_status_callback)(
                connection_status_callback,
                error_code_ptr,
                error_message_ptr,
                error_message.len() as c_long,
            )
        };

        let error_message = match extract_string_from_vec(error_message) {
            Ok(message) => message,
            Err(e) => {
                error!("Warning: error_message contains invalid UTF-8: {}", e);
                String::from("Invalid UTF-8 in error_message")
            }
        };

        let trans2quik_result = Trans2QuikResult::from(function_result);
        info!(
            "TRANS2QUIK_SET_CONNECTION_STATUS_CALLBACK -> {:?}, error_code: {}, error_message: {}",
            trans2quik_result, error_code, error_message
        );

        Ok(trans2quik_result)
    }

    /// Sets the callback function to receive information about the sent asynchronous transaction.
    pub fn set_transactions_reply_callback(&self) -> Result<Trans2QuikResult, Trans2QuikError> {
        let mut error_code: c_long = 0;
        let error_code_ptr = &mut error_code as *mut c_long;

        let mut error_message = vec![0 as c_char; 256];
        let error_message_ptr = error_message.as_mut_ptr() as *mut c_char;

        let function_result = unsafe {
            (self.trans2quik_set_transactions_reply_callback)(
                transaction_reply_callback,
                error_code_ptr,
                error_message_ptr,
                error_message.len() as c_long,
            )
        };

        let error_message = match extract_string_from_vec(error_message) {
            Ok(message) => message,
            Err(e) => {
                error!("Warning: error_message contains invalid UTF-8: {}", e);
                String::from("Invalid UTF-8 in error_message")
            }
        };

        let trans2quik_result = Trans2QuikResult::from(function_result);

        info!(
            "TRANS2QUIK_SET_TRANSACTIONS_REPLY_CALLBACK -> {:?}, error_code: {}, error_message: {}",
            trans2quik_result, error_code, error_message
        );

        Ok(trans2quik_result)
    }

    /// The function is used to create a list of classes and tools for subscribing to receive orders for them.
    pub fn subscribe_orders(
        &self,
        class_code: &str,
        sec_code: &str,
    ) -> Result<Trans2QuikResult, Trans2QuikError> {
        let class_code_c = CString::new(class_code)?;
        let class_code_ptr = class_code_c.as_ptr() as *mut c_char;

        let sec_code_c = CString::new(sec_code)?;
        let sec_code_ptr = sec_code_c.as_ptr() as *mut c_char;

        let function_result =
            unsafe { (self.trans2quik_subscribe_orders)(class_code_ptr, sec_code_ptr) };

        let trans2quik_result = Trans2QuikResult::from(function_result);

        info!(
            "TRANS2QUIK_SUBSCRIBE_ORDERS -> {:?}, class_code: {}, sec_code: {}",
            trans2quik_result, class_code, sec_code
        );

        Ok(trans2quik_result)
    }

    /// The function is used to create a list of classes and tools for subscribing to receive trades on them.
    pub fn subscribe_trades(
        &self,
        class_code: &str,
        sec_code: &str,
    ) -> Result<Trans2QuikResult, Trans2QuikError> {
        let class_code_c = CString::new(class_code)?;
        let class_code_ptr = class_code_c.as_ptr() as *mut c_char;

        let sec_code_c = CString::new(sec_code)?;
        let sec_code_ptr = sec_code_c.as_ptr() as *mut c_char;

        let function_result =
            unsafe { (self.trans2quik_subscribe_trades)(class_code_ptr, sec_code_ptr) };

        let trans2quik_result = Trans2QuikResult::from(function_result);

        info!(
            "TRANS2QUIK_SUBSCRIBE_TRADES -> {:?}, class_code: {}, sec_code: {}",
            trans2quik_result, class_code, sec_code
        );

        Ok(trans2quik_result)
    }

    /// The function starts the process of receiving requests for classes and tools defined
    /// by the TRANS2QUIK_SUBSCRIBE_ORDERS function.
    pub fn start_orders(&self) {
        unsafe { (self.trans2quik_start_orders)(order_status_callback) }
    }

    /// The function starts the process of receiving transactions with the parameters set
    /// by the function TRANS2QUIK_SUBSCRIBE_TRADES.
    pub fn start_trades(&self) {
        unsafe { (self.trans2quik_start_trades)(trade_status_callback) }
    }

    /// The function interrupts the operation of the TRANS2QUIK_START_ORDERS function and clears
    /// the list of received tools generated by the function
    /// TRANS2QUIK_SUBSCRIBE_ORDERS.
    pub fn unsubscribe_orders(&self) -> Result<Trans2QuikResult, Trans2QuikError> {
        let function_result = unsafe { (self.trans2quik_unsubscribe_orders)() };

        let trans2quik_result = Trans2QuikResult::from(function_result);

        info!("TRANS2QUIK_UNSUBSCRIBE_ORDERS -> {:?}", trans2quik_result);

        Ok(trans2quik_result)
    }

    /// The function interrupts the operation of the TRANS2QUIK_START_TRADES function and clears
    /// the list of received tools generated by the function
    /// TRANS2QUIK_SUBSCRIBE_TRADES.
    pub fn unsubscribe_trades(&self) -> Result<Trans2QuikResult, Trans2QuikError> {
        let function_result = unsafe { (self.trans2quik_unsubscribe_trades)() };

        let trans2quik_result = Trans2QuikResult::from(function_result);

        info!("TRANS2QUIK_UNSUBSCRIBE_TRADES -> {:?}", trans2quik_result);

        Ok(trans2quik_result)
    }
}

/// Loads the symbol from the library Trans2QUIK.dll
fn load_symbol<T>(library: &Library, name: &[u8]) -> Result<T, LibloadingError>
where
    T: Copy,
{
    unsafe {
        let symbol: Symbol<T> = library.get(name)?;
        Ok(*symbol)
    }
}

/// Extract String from `Vec<i8>`.
fn extract_string_from_vec(vec_i8: Vec<i8>) -> Result<String, FromUtf8Error> {
    let vec_u8: Vec<u8> = vec_i8.into_iter().map(|byte| byte as u8).collect();

    let null_pos = vec_u8
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(vec_u8.len());

    let vec_u8_trimmed = &vec_u8[..null_pos];

    let (decoded_str, _, _) = WINDOWS_1251.decode(vec_u8_trimmed);

    Ok(decoded_str.into_owned())
}

/// Callback function for status monitoring connections.
unsafe extern "C" fn connection_status_callback(
    connection_event: c_long,
    error_code: c_long,
    error_message: *mut c_char,
) {
    let error_message = if !error_message.is_null() {
        let c_str = CStr::from_ptr(error_message);
        let bytes = c_str.to_bytes();

        match WINDOWS_1251.decode(bytes) {
            (decoded_str, _, _) => decoded_str.to_owned().into_owned(),
        }
    } else {
        String::from("error_message is null")
    };

    let trans2quik_result = Trans2QuikResult::from(connection_event);

    info!(
        "TRANS2QUIK_CONNECTION_STATUS_CALLBACK -> {:?}, error_code: {}, error_message: {}",
        trans2quik_result, error_code, error_message
    );
}

/// Callback function for processing the received transaction information.
/// Attention! The submission of asynchronous transactions using
/// the callback function and synchronous transactions at the same time is prohibited.
/// This is due to the fact that it is impossible to correctly call
/// the callback function at a time when the synchronous transaction processing function has
/// not finished its work yet.
unsafe extern "C" fn transaction_reply_callback(
    result_code: c_long,
    error_code: c_long,
    reply_code: c_long,
    trans_id: c_long,
    order_num: c_ulonglong,
    error_message: *mut c_char,
    _trans_reply_descriptor: *mut intptr_t,
) {
    let error_message = if !error_message.is_null() {
        let c_str = CStr::from_ptr(error_message);
        let bytes = c_str.to_bytes();

        match WINDOWS_1251.decode(bytes) {
            (decoded_str, _, _) => decoded_str.to_owned().into_owned(),
        }
    } else {
        String::from("error_message is null")
    };

    let trans2quik_result = Trans2QuikResult::from(result_code);

    info!("TRANS2QUIK_TRANSACTION_REPLY_CALLBACK -> {:?}, error_code: {}, reply_code: {}, trans_id: {}, order_num: {}, error_message: {}", trans2quik_result, error_code, reply_code, trans_id, order_num, error_message);

    let (lock, cvar) = TRANSACTION_CALLBACK_RECEIVED.as_ref();
    let mut received = lock.lock().unwrap();
    *received = true;
    cvar.notify_one();
}

/// Callback function to get information about the order parameters.
unsafe extern "C" fn order_status_callback(
    mode: c_long,
    trans_id: c_long,
    order_num: c_ulonglong,
    class_code: *mut c_char,
    sec_code: *mut c_char,
    price: c_double,
    balance: i64,
    value: c_double,
    is_sell: c_long,
    status: c_long,
    _order_descriptor: intptr_t,
) {
    let mode = Mode::from(mode);
    let trans_id = TransId::from(trans_id);

    let class_code = if !class_code.is_null() {
        let c_str = CStr::from_ptr(class_code);
        let bytes = c_str.to_bytes();

        match WINDOWS_1251.decode(bytes) {
            (decoded_str, _, _) => decoded_str.to_owned().into_owned(),
        }
    } else {
        String::from("class_code is null")
    };

    let sec_code = if !sec_code.is_null() {
        let c_str = CStr::from_ptr(sec_code);
        let bytes = c_str.to_bytes();

        match WINDOWS_1251.decode(bytes) {
            (decoded_str, _, _) => decoded_str.to_owned().into_owned(),
        }
    } else {
        String::from("sec_code is null")
    };

    let is_sell = IsSell::from(is_sell);
    let status = Status::from(status);

    info!("TRANS2QUIK_ORDER_STATUS_CALLBACK -> mode: {:?}, trans_id: {:?}, order_num: {}, class_code: {}, sec_code: {}, price: {}, balance: {}, value: {}, is_sell: {:?}, status: {:?}", mode, trans_id, order_num, class_code, sec_code, price, balance, value, is_sell, status);

    let (lock, cvar) = ORDER_CALLBACK_RECEIVED.as_ref();
    let mut received = lock.lock().unwrap();
    *received = true;
    cvar.notify_one();
}

/// Callback function to get information about the transaction.
unsafe extern "C" fn trade_status_callback(
    mode: c_long,
    trade_num: c_ulonglong,
    order_num: c_ulonglong,
    class_code: *mut c_char,
    sec_code: *mut c_char,
    price: c_double,
    quantity: i64,
    is_sell: c_long,
    value: c_double,
    _trade_descriptor: intptr_t,
) {
    let mode = Mode::from(mode);

    let class_code = if !class_code.is_null() {
        let c_str = CStr::from_ptr(class_code);
        let bytes = c_str.to_bytes();

        match WINDOWS_1251.decode(bytes) {
            (decoded_str, _, _) => decoded_str.to_owned().into_owned(),
        }
    } else {
        String::from("class_code is null")
    };

    let sec_code = if !sec_code.is_null() {
        let c_str = CStr::from_ptr(sec_code);
        let bytes = c_str.to_bytes();

        match WINDOWS_1251.decode(bytes) {
            (decoded_str, _, _) => decoded_str.to_owned().into_owned(),
        }
    } else {
        String::from("sec_code is null")
    };

    let is_sell = IsSell::from(is_sell);

    info!("TRANS2QUIK_TRADE_STATUS_CALLBACK -> mode: {:?}, trade_num: {}, order_num: {}, class_code: {}, sec_code: {}, price: {}, quantity: {}, is_sell: {:?}, value: {}", mode, trade_num, order_num, class_code, sec_code, price, quantity, is_sell, value);

    let (lock, cvar) = TRADE_CALLBACK_RECEIVED.as_ref();
    let mut received = lock.lock().unwrap();
    *received = true;
    cvar.notify_one();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trans2quik_result_conversion() {
        assert_eq!(Trans2QuikResult::from(0), Trans2QuikResult::Success);
        assert_eq!(Trans2QuikResult::from(1), Trans2QuikResult::Failed);
        assert_eq!(
            Trans2QuikResult::from(2),
            Trans2QuikResult::TerminalNotFound
        );
        assert_eq!(
            Trans2QuikResult::from(3),
            Trans2QuikResult::DllVersionNotSupported
        );
        assert_eq!(
            Trans2QuikResult::from(4),
            Trans2QuikResult::AlreadyConnectedToQuik
        );
        assert_eq!(Trans2QuikResult::from(5), Trans2QuikResult::WrongSyntax);
        assert_eq!(
            Trans2QuikResult::from(6),
            Trans2QuikResult::QuikNotConnected
        );
        assert_eq!(Trans2QuikResult::from(7), Trans2QuikResult::DllNotConnected);
        assert_eq!(Trans2QuikResult::from(8), Trans2QuikResult::QuikConnected);
        assert_eq!(
            Trans2QuikResult::from(9),
            Trans2QuikResult::QuikDisconnected
        );
        assert_eq!(Trans2QuikResult::from(10), Trans2QuikResult::DllConnected);
        assert_eq!(
            Trans2QuikResult::from(11),
            Trans2QuikResult::DllDisconnected
        );
        assert_eq!(
            Trans2QuikResult::from(12),
            Trans2QuikResult::MemoryAllocationError
        );
        assert_eq!(
            Trans2QuikResult::from(13),
            Trans2QuikResult::WrongConnectionHandle
        );
        assert_eq!(
            Trans2QuikResult::from(14),
            Trans2QuikResult::WrongInputParams
        );
        assert_eq!(Trans2QuikResult::from(999), Trans2QuikResult::Unknown);
    }

    #[test]
    fn test_trans2quikerror_from_libloadingerror() {
        // Attempt to load a non-existent library to produce a LibloadingError
        let libloading_error = unsafe {
            match Library::new("/invalid/path/to/nonexistent/library") {
                Ok(_) => panic!("Expected an error, but library loaded successfully."),
                Err(e) => e,
            }
        };

        // Convert it into a Trans2QuikError
        let trans2quik_error: Trans2QuikError = Trans2QuikError::from(libloading_error);

        // Assert that it matches the expected enumeration variant
        if let Trans2QuikError::LibLoading(_) = trans2quik_error {
            // Passed: we correctly converted the error
        } else {
            panic!("Expected Trans2QuikError::LibLoading variant.");
        }
    }

    #[test]
    fn test_trans2quikerror_from_nulerror() {
        // Create a NulError by attempting to construct a CString with an embedded null byte
        let nul_err = CString::new("Invalid\0String").unwrap_err();

        // Convert it into a Trans2QuikError
        let trans2quik_error: Trans2QuikError = Trans2QuikError::from(nul_err);

        // Assert that it matches the expected enumeration variant
        matches!(trans2quik_error, Trans2QuikError::NulError(_));
    }

    #[test]
    fn test_display_for_trans2quikerror() {
        // Test conversion and display message for NulError
        let nul_err = CString::new("Invalid\0String").unwrap_err();
        let trans2quik_error_nul: Trans2QuikError = Trans2QuikError::from(nul_err);

        // Test display format for NulError
        let expected_display_nul = format!("{:?}", trans2quik_error_nul);
        assert_eq!(expected_display_nul, format!("{}", trans2quik_error_nul));

        // For LibloadingError: simulate a common error scenario
        // Open a library with an invalid path to trigger a DlOpen error
        let libloading_error = unsafe {
            match Library::new("/invalid/path/to/nonexistent/lib") {
                Ok(_) => panic!("Expected an error, but library loaded successfully"),
                Err(e) => e,
            }
        };
        let trans2quik_error_lib: Trans2QuikError = Trans2QuikError::from(libloading_error);

        // Test display format for LibLoading error
        let expected_display_lib = format!("{:?}", trans2quik_error_lib);
        assert_eq!(expected_display_lib, format!("{}", trans2quik_error_lib));
    }
}
