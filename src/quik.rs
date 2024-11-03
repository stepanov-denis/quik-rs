use std::error::Error;
use libloading;
use libc;
use std::ptr;
use std::ffi::CStr;
use std::ffi::CString;
use libloading::{Library, Symbol};
use libc::{c_char, c_long, c_ulong, c_void};
use tracing::{info, error};
use tracing_subscriber;
use std::fmt;


type TRANS2QUIK_CONNECTION_STATUS_CALLBACK = ();


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
        let symbol: Symbol<T> = library.get(name)?;
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

    /// Prototype of a callback function for status monitoring connections.
    trans2quik_connection_status_callback: unsafe extern "C" fn(*mut c_long, *mut c_long, *mut c_char) -> c_void,

    // А callback function for processing the received connection information.
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

        // Prototype of a callback function for status monitoring connections.
        let trans2quik_connection_status_callback = load_symbol::<unsafe extern "C" fn(
            *mut c_long,
            *mut c_long,
            *mut c_char,
        ) -> c_void>(&library, b"TRANS2QUIK_CONNECTION_STATUS_CALLBACK\0")?;
        
        Ok(Terminal {
            library,
            trans2quik_connect,
            trans2quik_disconnect,
            trans2quik_is_quik_connected,
            trans2quik_is_dll_connected,
            trans2quik_connection_status_callback,
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
        let mut result_code: c_long = 0;
        let mut result_message = vec![0 as c_char; 256];
        let result_message_len = result_message.len() as c_ulong;

        // Вызов функции
        let function_result = func(
            &mut result_code,
            result_message.as_mut_ptr(),
            result_message_len,
        );

        let result_message = unsafe {
            CStr::from_ptr(result_message.as_ptr())
                .to_string_lossy()
                .into_owned()
        };
        let trans2quik_result = Trans2quikResult::from(function_result);
        println!(
            "{} -> {:?} {}",
            function_name, trans2quik_result, result_message
        );
        Ok(trans2quik_result)
    }


    /// The function is used to establish communication with the QUIK terminal.
    pub fn connect(&self) -> Result<Trans2quikResult, Box<dyn std::error::Error>> {
        let connection_string = CString::new(r"c:\QUIK Junior")?;

        let function = |result_code: &mut c_long,
                        result_message_ptr: *mut c_char,
                        result_message_len: c_ulong| unsafe {
            (self.trans2quik_connect)(
                connection_string.as_ptr(),
                result_code,
                result_message_ptr,
                result_message_len,
            )
        };

        self.call_trans2quik_function("TRANS2QUIK_CONNECT", function)
    }


    /// The function is used to disconnect from the QUIK terminal.
    pub fn disconnect(&self) -> Result<Trans2quikResult, Box<dyn std::error::Error>> {
        let function = |result_code: &mut c_long,
                        result_message_ptr: *mut c_char,
                        result_message_len: c_ulong| unsafe {
            (self.trans2quik_disconnect)(
                result_code,
                result_message_ptr,
                result_message_len,
            )
        };

        self.call_trans2quik_function("TRANS2QUIK_DISCONNECT", function)
    }


    /// The function is used to check if there is a connection between the QUIK terminal and the server.
    pub fn is_quik_connected(&self) -> Result<Trans2quikResult, Box<dyn std::error::Error>> {
        let function = |result_code: &mut c_long,
                        result_message_ptr: *mut c_char,
                        result_message_len: c_ulong| unsafe {
            (self.trans2quik_is_quik_connected)(
                result_code,
                result_message_ptr,
                result_message_len,
            )
        };

        self.call_trans2quik_function("TRANS2QUIK_IS_QUIK_CONNECTED", function)
    }


    /// Checking for a connection between the library Trans2QUIK.dll and the QUIK terminal.
    pub fn is_dll_connected(&self) -> Result<Trans2quikResult, Box<dyn std::error::Error>> {
        let function = |result_code: &mut c_long,
                        result_message_ptr: *mut c_char,
                        result_message_len: c_ulong| unsafe {
            (self.trans2quik_is_dll_connected)(
                result_code,
                result_message_ptr,
                result_message_len,
            )
        };

        self.call_trans2quik_function("TRANS2QUIK_IS_DLL_CONNECTED", function)
    }


    /// Prototype of a callback function for status monitoring connections.
    pub fn connection_status_callback(&self) -> Result<Trans2quikResult, Box<dyn std::error::Error>> {
        let mut connection_event: c_long = -1;
        let mut result_code: c_long = 0;
        let mut result_message = vec![0 as c_char; 256];

        // Вызов функции
        let function_result = unsafe {
            (self.trans2quik_connection_status_callback)(
                &mut connection_event as *mut c_long,
                &mut result_code as *mut c_long,
                result_message.as_mut_ptr(),
            )
        } as c_long;

        let result_message = unsafe {
            CStr::from_ptr(result_message.as_ptr())
                .to_string_lossy()
                .into_owned()
        };
        let trans2quik_result = Trans2quikResult::from(function_result);
        println!(
            "-> {:?} {}",
            trans2quik_result, result_message
        );
        Ok(trans2quik_result)
    }
}