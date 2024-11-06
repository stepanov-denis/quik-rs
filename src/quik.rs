use libloading;
use libc;
use std::ffi::CStr;
use std::ffi::CString;
use libloading::{Library, Symbol};
use libc::{c_char, c_long, c_ulong, c_double};
use tracing::{info, error};
use std::str;


// Этот тип может иметь разную ширину в зависимости от платформы, 
// поэтому используем cfg для определения типа.
#[cfg(target_pointer_width = "64")]
type IntPtr = i64;


type SubsribeOrders = unsafe extern "C" fn(*const c_char, *const c_char) -> c_long;


type Trans2QuikConnectionStatusCallback = unsafe extern "C" fn(
    connection_event: *mut c_long,
    error_code: *mut c_long,
    error_message: *mut c_char,
);


/// Corresponds to the description of constants whose values are returned when exiting functions
/// and procedures in the library `Trans2QUIK.dll `:
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
#[derive(Debug)]
#[repr(i32)]
pub enum Trans2quikResult {
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


/// Implementation From<c_long> for Trans2quikResult,
/// to automatically convert the integer code to the appropriate enumeration variant:
impl From<c_long> for Trans2quikResult {
    fn from(code: c_long) -> Self {
        match code {
            0 => Trans2quikResult::Success,
            1 => Trans2quikResult::Failed,
            2 => Trans2quikResult::TerminalNotFound,
            3 => Trans2quikResult::DllVersionNotSupported,
            4 => Trans2quikResult::AlreadyConnectedToQuik,
            5 => Trans2quikResult::WrongSyntax,
            6 => Trans2quikResult::QuikNotConnected,
            7 => Trans2quikResult::DllNotConnected,
            8 => Trans2quikResult::QuikConnected,
            9 => Trans2quikResult::QuikDisconnected,
            10 => Trans2quikResult::DllConnected,
            11 => Trans2quikResult::DllDisconnected,
            12 => Trans2quikResult::MemoryAllocationError,
            13 => Trans2quikResult::WrongConnectionHandle,
            14 => Trans2quikResult::WrongInputParams,
            _ => Trans2quikResult::Unknown,
        }
    }
}


/// Loads the symbol from the library Trans2QUIK.dll
fn load_symbol<T>(
    library: &Library,
    name: &[u8],
) -> Result<T, Box<dyn std::error::Error>>
where
    T: Copy,
{
    unsafe {
        let symbol: Symbol<T> = library.get(name).map_err(|e| { error!("Load '{}' from `Trans2QUIK.dll` error: {}", str::from_utf8_unchecked(name), e); e})?;
        Ok(*symbol)
    }
}


/// The `Terminal` structure is used to interact with the QUIK trading terminal through the library `Trans2QUIK.dll`.
///
/// This structure provides loading of the DLL library `Trans2QUIK.dll `, establishing a connection to the QUIK terminal
/// and calling functions from the library to control the terminal and perform trading operations.
///
/// # Example of use
/// ```
/// let path = r"c:\QUIK Junior\trans2quik.dll";
/// let terminal = quik::Terminal::new(path)?;
/// terminal.connect()?;
/// ```
pub struct Terminal {
    /// Loading a dynamic library Trans2QUIK.dll , which provides an API for interacting with QUIK.
    library: Library,

    /// Calling a function from the library Trans2QUIK.dll for establishing communication with the QUIK terminal.
    trans2quik_connect: unsafe extern "C" fn(*const c_char, *mut c_long, *mut c_char, c_ulong) -> c_long,

    /// Calling a function from the library Trans2QUIK.dll to disconnecting from the QUIK terminal.
    trans2quik_disconnect: unsafe extern "C" fn(*mut c_long, *mut c_char, c_ulong) -> c_long,

    /// Calling a function from the library Trans2QUIK.dll to check for a connection between the QUIK terminal and the server.
    trans2quik_is_quik_connected: unsafe extern "C" fn(*mut c_long, *mut c_char, c_ulong) -> c_long,

    /// Calling a function from the library Trans2QUIK.dll to check if there is a connection between the library Trans2QUIK.dll and the QUIK terminal.
    trans2quik_is_dll_connected: unsafe extern "C" fn(*mut c_long, *mut c_char, c_ulong) -> c_long,

    /// А callback function for processing the received connection information.
    trans2quik_set_connection_status_callback: unsafe extern "C" fn(Trans2QuikConnectionStatusCallback, *mut c_long, *mut c_char, c_ulong) -> c_long,

    /// Синхронная отправка транзакции. При синхронной отправке возврат из функции происходит 
    /// только после получения результата выполнения транзакции, либо после разрыва связи 
    /// терминала QUIK с сервером.
    trans2quik_send_sync_transaction: unsafe extern "C" fn(
        *const c_char,
        *mut c_long,
        *mut c_ulong,
        *mut c_double,
        *mut c_char,
        c_ulong,
        *mut c_long,
        *mut c_char,
        c_ulong
    ) -> c_long,
}


impl Terminal {
    /// The function is used to load the library Trans2QUIK.dll.
    pub fn new(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        // Loading a dynamic library `Trans2QUIK.dll `, which provides an API for interacting with QUIK.
        let library = unsafe { Library::new(path)? };

        // Calling a function from the library Trans2QUIK.dll for establishing communication with the QUIK terminal.
        let trans2quik_connect = load_symbol::<unsafe extern "C" fn(
            *const c_char,
            *mut c_long,
            *mut c_char,
            c_ulong,
        ) -> c_long>(&library, b"TRANS2QUIK_CONNECT\0")?;

        // Calling a function from the library Trans2QUIK.dll to disconnecting from the QUIK terminal.
        let trans2quik_disconnect = load_symbol::<unsafe extern "C" fn(
            *mut c_long,
            *mut c_char,
            c_ulong,
        ) -> c_long>(&library, b"TRANS2QUIK_DISCONNECT\0")?;

        // Calling a function from the library Trans2QUIK.dll to check for a connection between the QUIK terminal and the server.
        let trans2quik_is_quik_connected = load_symbol::<unsafe extern "C" fn(
            *mut c_long,
            *mut c_char,
            c_ulong,
        ) -> c_long>(&library, b"TRANS2QUIK_IS_QUIK_CONNECTED\0")?;

        // Calling a function from the library Trans2QUIK.dll to check if there is a connection between the library Trans2QUIK.dll and the QUIK terminal.
        let trans2quik_is_dll_connected = load_symbol::<unsafe extern "C" fn(
            *mut c_long,
            *mut c_char,
            c_ulong,
        ) -> c_long>(&library, b"TRANS2QUIK_IS_DLL_CONNECTED\0")?;

        // А callback function for processing the received connection information.
        let trans2quik_set_connection_status_callback = load_symbol::<unsafe extern "C" fn(
            Trans2QuikConnectionStatusCallback,
            *mut c_long,
            *mut c_char,
            c_ulong,
        ) -> c_long>(&library, b"TRANS2QUIK_SET_CONNECTION_STATUS_CALLBACK\0")?;

        // Синхронная отправка транзакции. При синхронной отправке возврат из функции происходит 
        // только после получения результата выполнения транзакции, либо после разрыва связи 
        // терминала QUIK с сервером.
        let trans2quik_send_sync_transaction = load_symbol::<unsafe extern "C" fn(
            *const c_char,
            *mut c_long,
            *mut c_ulong,
            *mut c_double,
            *mut c_char,
            c_ulong,
            *mut c_long,
            *mut c_char,
            c_ulong
        ) -> c_long>(&library, b"TRANS2QUIK_SEND_SYNC_TRANSACTION\0")?;
        
        Ok(Terminal {
            library,
            trans2quik_connect,
            trans2quik_disconnect,
            trans2quik_is_quik_connected,
            trans2quik_is_dll_connected,
            trans2quik_set_connection_status_callback,
            trans2quik_send_sync_transaction,
        })
    }


    /// Calling a function from the library Trans2QUIK.dll.
    fn call_trans2quik_function<F>(
        &self,
        function_name: &str,
        func: F,
    ) -> Result<Trans2quikResult, Box<dyn std::error::Error>>
    where
        F: FnOnce(&mut c_long, *mut c_char, c_ulong) -> c_long,
    {
        let mut error_code: c_long = 0;
        let mut error_message = vec![0 as c_char; 256];
        let error_message_len = error_message.len() as c_ulong;

        // Вызов функции
        let function_result = func(
            &mut error_code,
            error_message.as_mut_ptr(),
            error_message_len,
        );

        let error_message = unsafe {
            CStr::from_ptr(error_message.as_ptr())
                .to_string_lossy()
                .into_owned()
        };
        let trans2quik_result = Trans2quikResult::from(function_result);
        info!(
            "{} -> {:?}, error_code: {}, error_message: {}",
            function_name, trans2quik_result, error_code, error_message
        );
        Ok(trans2quik_result)
    }


    /// The function is used to establish communication with the QUIK terminal.
    pub fn connect(&self) -> Result<Trans2quikResult, Box<dyn std::error::Error>> {
        let connection_string = CString::new(r"c:\QUIK Junior")?;

        let function = |error_code: &mut c_long,
                        error_message: *mut c_char,
                        error_message_len: c_ulong| unsafe {
            (self.trans2quik_connect)(
                connection_string.as_ptr(),
                error_code,
                error_message,
                error_message_len,
            )
        };

        self.call_trans2quik_function("TRANS2QUIK_CONNECT", function)
    }


    /// The function is used to disconnect from the QUIK terminal.
    pub fn disconnect(&self) -> Result<Trans2quikResult, Box<dyn std::error::Error>> {
        let function = |error_code: &mut c_long,
                        error_message: *mut c_char,
                        error_message_len: c_ulong| unsafe {
            (self.trans2quik_disconnect)(
                error_code,
                error_message,
                error_message_len,
            )
        };

        self.call_trans2quik_function("TRANS2QUIK_DISCONNECT", function)
    }


    /// The function is used to check if there is a connection between the QUIK terminal and the server.
    pub fn is_quik_connected(&self) -> Result<Trans2quikResult, Box<dyn std::error::Error>> {
        let function = |error_code: &mut c_long,
                        error_message: *mut c_char,
                        error_message_len: c_ulong| unsafe {
            (self.trans2quik_is_quik_connected)(
                error_code,
                error_message,
                error_message_len,
            )
        };

        self.call_trans2quik_function("TRANS2QUIK_IS_QUIK_CONNECTED", function)
    }


    /// Checking for a connection between the library Trans2QUIK.dll and the QUIK terminal.
    pub fn is_dll_connected(&self) -> Result<Trans2quikResult, Box<dyn std::error::Error>> {
        let function = |error_code: &mut c_long,
                        error_message: *mut c_char,
                        error_message_len: c_ulong| unsafe {
            (self.trans2quik_is_dll_connected)(
                error_code,
                error_message,
                error_message_len,
            )
        };

        self.call_trans2quik_function("TRANS2QUIK_IS_DLL_CONNECTED", function)
    }


    /// А callback function for processing the received connection information.
    pub fn set_connection_status_callback(&mut self) -> Result<Trans2quikResult, Box<dyn std::error::Error>> {
        let mut error_code: c_long = 0;
        let mut error_message = vec![0 as c_char; 256];
        let error_message_len = error_message.len() as c_ulong;

        let function_result = unsafe {
            (self.trans2quik_set_connection_status_callback)(
                connection_status_callback,
                &mut error_code,
                error_message.as_mut_ptr(),
                error_message_len,
            )
        };

        let error_message = unsafe {
            CStr::from_ptr(error_message.as_ptr())
                .to_string_lossy()
                .into_owned()
        };

        let trans2quik_result = Trans2quikResult::from(function_result);
        info!(
            "TRANS2QUIK_SET_CONNECTION_STATUS_CALLBACK -> {:?}, error_code: {}, error_message: {}",
            trans2quik_result, error_code, error_message
        );

        Ok(trans2quik_result)
    }


    /// Синхронная отправка транзакции. При синхронной отправке возврат из функции происходит 
    /// только после получения результата выполнения транзакции, либо после разрыва связи 
    /// терминала QUIK с сервером.
    pub fn send_sync_transaction(&self, transaction_str: &str) -> Result<Trans2quikResult, Box<dyn std::error::Error>> {
        let transaction_str = CString::new(transaction_str).expect("CString::new failed");
        let mut reply_code: c_long = 0;
        let mut trans_id: c_ulong = 0;
        let mut order_num: c_double = 0.0;
        let mut result_message = vec![0 as c_char; 256];
        let mut error_code: c_long = 0;
        let mut error_message = vec![0 as c_char; 256];

        let function_result = unsafe {
            (self.trans2quik_send_sync_transaction)(
                transaction_str.as_ptr(),
                &mut reply_code as &mut c_long,
                &mut trans_id as &mut c_ulong,
                &mut order_num as &mut c_double,
                result_message.as_mut_ptr(),
                result_message.len() as c_ulong,
                &mut error_code as &mut c_long,
                error_message.as_mut_ptr(),
                error_message.len() as c_ulong,
            )
        };

        let result_message = unsafe {
            CStr::from_ptr(result_message.as_ptr())
                .to_string_lossy()
                .into_owned()
        };

        let error_message = unsafe {
            CStr::from_ptr(error_message.as_ptr())
                .to_string_lossy()
                .into_owned()
        };

        let trans2quik_result = Trans2quikResult::from(function_result);

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
}


/// Prototype of a callback function for status monitoring connections.
unsafe extern "C" fn connection_status_callback(connection_event: *mut c_long, error_code: *mut c_long, error_message: *mut c_char) {
    info!("event: {:?}", connection_event);
}