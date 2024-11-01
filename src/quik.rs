use std::error::Error;
use libloading;
use libc;
use std::ptr;
use std::ffi::CStr;
use std::ffi::CString;
use libloading::{Library, Symbol};
use libc::{c_char, c_long, c_ulong};
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

/// The `Terminal` structure is used to interact with the QUIK trading terminal through the library `Trans2QUIK.dll `.
///
/// This structure provides loading of the DLL library `Trans2QUIK.dll `, establishing a connection to the QUIK terminal
/// and calling functions from the library to control the terminal and perform trading operations.
///
/// # Example of use
/// ``
/// let path = r"c:\QUIK Junior\trans2quik.dll";
/// let terminal = quik::Terminal::new(path)?;
/// terminal.connect()?;
/// ```
pub struct Terminal {
    /// Loading a dynamic library `Trans2QUIK.dll `, which provides an API for interacting with QUIK.
    library: Library,
}


impl Terminal {
    /// The function is used to load the library Trans2QUIK.dll .
    pub fn new(path: &str) -> Result<Self, libloading::Error> {
        unsafe {
            let library = Library::new(path).map_err(|e| { error!("DLL loading error: {:?}", e); e})?;
            
            Ok(Terminal { library })
        }
    }


    /// The function is used to call from the library Trans2QUIK.dll functions for establishing communication with the QUIK terminal.
    fn trans2quik_connect(&self, connection_string: *const c_char, result_code: *mut c_long, result_message: *mut c_char, result_message_len: c_ulong) -> Result<c_long, libloading::Error> {
        unsafe {
            let connect: Symbol<unsafe extern "C" fn(*const c_char, *mut c_long, *mut c_char, c_ulong) -> c_long> = self.library.get(b"TRANS2QUIK_CONNECT\0").map_err(|e| { error!("TRANS2QUIK_CONNECT error: {}", e); e})?;

            Ok(connect(connection_string, result_code, result_message, result_message_len))
        }
    }

    /// The function is used to establish communication with the QUIK terminal.
    pub fn connect(&self) -> Result<Trans2quikResult, libloading::Error> {
        // Prepare the parameters
        let connection_string = CString::new(r"c:\QUIK Junior").expect("CString::new failed");
        let mut result_code: c_long = -1;
        let mut result_message = vec![0 as c_char; 256];
        let mut result_message_len = result_message.len();
    
        // Call the function
        let function_result = self.trans2quik_connect(
            connection_string.as_ptr(),
            &mut result_code as *mut c_long,
            result_message.as_mut_ptr(),
            result_message_len as c_ulong,
        )?;
    
        // Convert the result message
        let result_message = unsafe {
            CStr::from_ptr(result_message.as_ptr()).to_string_lossy().into_owned()
        };
    
        // Map the result_code to Trans2quikResult
        let trans2quik_result = Trans2quikResult::from(function_result);
    
        // Log the result
        info!("TRANS2QUIK_CONNECT -> {:?}: {}", trans2quik_result, result_message);
    
        // Return the result
        Ok(trans2quik_result)
    }

    /// The function is used to call from the library Trans2QUIK.dll the functions of disconnecting from the QUIK terminal.
    fn trans2quik_disconnect(&self, result_code: *mut c_long, result_message: *mut c_char, result_message_len: c_ulong) -> Result<c_long, libloading::Error> {
        unsafe {
            let disconnect: Symbol<unsafe extern "C" fn(*mut c_long, *mut c_char, c_ulong) -> c_long> = self.library.get(b"TRANS2QUIK_DISCONNECT\0").map_err(|e| { error!("TRANS2QUIK_DISCONNECT error: {}", e); e})?;

            Ok(disconnect(result_code, result_message, result_message_len))
        }
    }

    /// The function is used to disconnect from the QUIK terminal.
    pub fn disconnect(&self) -> Result<Trans2quikResult, libloading::Error> {
        // Prepare the parameters
        let mut result_code: c_long = 0;
        let mut result_message = vec![0 as c_char; 256];
        let mut result_message_len = result_message.len();
    
        // Call the function
        let function_result = self.trans2quik_disconnect(
            &mut result_code as *mut c_long,
            result_message.as_mut_ptr(),
            result_message_len as c_ulong,
        )?;
    
        // Convert the result message
        let result_message = unsafe {
            CStr::from_ptr(result_message.as_ptr()).to_string_lossy().into_owned()
        };
    
        // Map the result_code to Trans2quikResult
        let trans2quik_result = Trans2quikResult::from(function_result);
    
        // Log the result
        info!("TRANS2QUIK_DISCONNECT -> {:?}: {}", trans2quik_result, result_message);
    
        // Return the result
        Ok(trans2quik_result)
    }


    /// The function is used to call the function to check for a connection between the QUIK terminal and the server.
    fn trans2quik_is_quik_connected(&self, result_code: *mut c_long, result_message: *mut c_char, result_message_len: c_ulong) -> Result<c_long, libloading::Error> {
        unsafe {
            let is_quik_connected: Symbol<unsafe extern "C" fn(*mut c_long, *mut c_char, c_ulong) -> c_long> = self.library.get(b"TRANS2QUIK_IS_QUIK_CONNECTED\0").map_err(|e| { error!("TRANS2QUIK_IS_QUIK_CONNECTED error: {}", e); e})?;

            Ok(is_quik_connected(result_code, result_message, result_message_len))
        }
    }


    /// The function is used to check if there is a connection between the QUIK terminal and the server.
    pub fn is_quik_connected(&self) -> Result<Trans2quikResult, libloading::Error> {
        // Prepare the parameters
        let mut result_code: c_long = 0;
        let mut result_message = vec![0 as c_char; 256];
        let result_message_len = result_message.len();
    
        // Call the function
        let function_result = self.trans2quik_is_quik_connected(
            &mut result_code as *mut c_long,
            result_message.as_mut_ptr(),
            result_message_len as c_ulong,
        )?;
    
        // Convert the result message
        let result_message = unsafe {
            CStr::from_ptr(result_message.as_ptr()).to_string_lossy().into_owned()
        };
    
        // Map the result_code to Trans2quikResult
        let trans2quik_result = Trans2quikResult::from(function_result);
    
        // Log the result
        info!("TRANS2QUIK_IS_QUIK_CONNECTED -> {:?}: {}", trans2quik_result, result_message);
    
        // Return the result
        Ok(trans2quik_result)
    }
}


/// Функция используется для проверки наличия соединения между библиотекой Trans2QUIK.dll и терминалом QUIK.
pub fn is_dll_connected(lib: &Library) -> bool {
    // Определяем тип функции
    unsafe {
        // Найдем функцию TRANS2QUIK_IS_DLL_CONNECTED в библиотеке
        let is_dll_connected: Symbol<unsafe extern "C" fn(*mut c_long, *mut c_char, c_ulong) -> c_long> = lib.get(b"TRANS2QUIK_IS_DLL_CONNECTED\0").expect("Could not find function");
        
        // Вызываем функцию
        let mut error_code: c_long = 0;
        let mut error_message = vec![0 as c_char; 256];

        let result = is_dll_connected(
            &mut error_code as *mut c_long,
            error_message.as_mut_ptr(),
            error_message.len() as c_ulong
        );

        let error_message = CStr::from_ptr(error_message.as_ptr()).to_string_lossy().into_owned();

        match result {
            10 => info!(" TRANS2QUIK_DLL_CONNECTED - соединение библиотеки Trans2QUIK.dll с терминалом QUIK установлено"),
            7 => {
                info!("TRANS2QUIK_DLL_NOT_CONNECTED - не установлена связь библиотеки Trans2QUIK.dll с терминалом QUIK");
                info!("Error code: {}, error message: {}", error_code, error_message);
            },
            _ => info!("Unknown result code"),
        }

        if result == 10
        {
            return true
        }
        else
        {
            return false
        }
    }
}


/// Описание прототипа Функции обратного вызова для контроля за состоянием соединения 
/// между библиотекой Trans2QUIK.dll и используемым терминалом QUIK и между 
/// используемым терминалом QUIK и сервером.
pub fn connection_status_callback(lib: &Library) {
    // Определяем тип функции
    unsafe {
        // Найдем функцию TRANS2QUIK_CONNECTION_STATUS_CALLBACK в библиотеке
        let connection_status_callback: Symbol<unsafe extern "C" fn(*mut c_long, *mut c_char) -> c_long> = lib.get(b"TRANS2QUIK_CONNECTION_STATUS_CALLBACK\0").expect("Could not find function");
        
        // Вызываем функцию
        let mut error_code: c_long = 0;
        let mut error_message = vec![0 as c_char; 256];

        let result = connection_status_callback(
            &mut error_code as *mut c_long,
            error_message.as_mut_ptr()
        );

        let error_message = CStr::from_ptr(error_message.as_ptr()).to_string_lossy().into_owned();

        match result {
            8 => info!("TRANS2QUIK_QUIK_CONNECTED - соединение между терминалом QUIK и сервером установлено"),
            9 => {
                info!("TRANS2QUIK_QUIK_DISCONNECTED - соединение между терминалом QUIK и сервером разорвано");
                info!("Error code: {}, error message: {}", error_code, error_message);
            },
            10 => info!(" TRANS2QUIK_DLL_CONNECTED - соединение между DLL и используемым терминалом QUIK установлено"),
            11 => {
                info!(" TRANS2QUIK_DLL_DISCONNECTED - соединение между DLL и используемым терминалом QUIK разорвано");
                info!("Error code: {}, error message: {}", error_code, error_message);
            }
            _ => info!("Unknown result code"),
        }
    }
}


/// Описание прототипа функции обратного вызова для обработки полученной информации о соединении
pub fn set_connection_status_callback(lib: &Library) -> bool {
    // Определяем тип функции
    unsafe {
        // Найдем функцию  TRANS2QUIK_SET_CONNECTION_STATUS_CALLBACK в библиотеке
        let set_connection_status_callback: Symbol<unsafe extern "C" fn(Option<TRANS2QUIK_CONNECTION_STATUS_CALLBACK>, *mut c_long, *mut c_char, c_ulong) -> c_long> = lib.get(b"TRANS2QUIK_SET_CONNECTION_STATUS_CALLBACK\0").expect("Could not find function");
        
        // Вызываем функцию
        let mut error_code: c_long = 0;
        let mut error_message = vec![0 as c_char; 256];

        let result = set_connection_status_callback(
            Some(connection_status_callback(&lib)),
            &mut error_code as *mut c_long,
            error_message.as_mut_ptr(),
            error_message.len() as c_ulong
        );

        let error_message = CStr::from_ptr(error_message.as_ptr()).to_string_lossy().into_owned();

        match result {
            0 => info!("TRANS2QUIK_SUCCESS - функция обратного вызова установлена"),
            1 => {
                info!("TRANS2QUIK_FAILED - функцию обратного вызова установить не удалось");
                info!("Error code: {}, error message: {}", error_code, error_message);
            },
            _ => info!("Unknown result code"),
        }

        if result == 10
        {
            return true
        }
        else
        {
            return false
        }
    }
}