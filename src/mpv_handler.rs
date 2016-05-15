use mpv_gen::{mpv_command, mpv_command_async, mpv_wait_event, mpv_create, mpv_initialize,
              mpv_terminate_destroy, mpv_handle, mpv_set_option,
              Struct_mpv_event, mpv_set_property, mpv_set_property_async, mpv_get_property,
              mpv_get_property_async, mpv_opengl_cb_get_proc_address_fn, mpv_get_sub_api,
              mpv_opengl_cb_context,
              MpvFormat as MpvInternalFormat};
use mpv_enums::*;
use mpv_error::*;
use mpv_gl::OpenGLContext;

use std::os::raw::{c_void,c_char};
use std::ffi::CStr;
use std::{ffi, ptr};
use std::mem;

pub struct MpvHandler {
    handle: *mut mpv_handle,
}

pub trait MpvFormat {
    fn call_as_c_void<F : FnMut(*mut c_void)>(&self,f:F);
    fn get_from_c_void<F : FnMut(*mut c_void)>(mut f:F) -> Self;
    fn get_mpv_format() -> MpvInternalFormat ;
}

impl MpvFormat for f64 {
    fn call_as_c_void<F : FnMut(*mut c_void)>(&self,mut f:F){
        let mut cpy : f64= *self;
        let pointer = &mut cpy as *mut _ as *mut c_void;
        f(pointer)
    }

    fn get_from_c_void<F : FnMut(*mut c_void)>(mut f:F) -> f64 {
        let mut ret_value = f64::default() ;
        let pointer = &mut ret_value as *mut _ as *mut c_void;
        f(pointer);
        ret_value
    }

    fn get_mpv_format() -> MpvInternalFormat {
        MpvInternalFormat::MPV_FORMAT_DOUBLE
    }
}

impl MpvFormat for i64 {
    fn call_as_c_void<F : FnMut(*mut c_void)>(&self,mut f:F){
        let format = Self::get_mpv_format();
        let mut cpy : i64= *self;
        let pointer = &mut cpy as *mut _ as *mut c_void;
        f(pointer)
    }

    fn get_from_c_void<F : FnMut(*mut c_void)>(mut f:F) -> i64 {
        let mut ret_value = i64::default() ;
        let pointer = &mut ret_value as *mut _ as *mut c_void;
        f(pointer);
        ret_value
    }

    fn get_mpv_format() -> MpvInternalFormat {
        MpvInternalFormat::MPV_FORMAT_INT64
    }
}

impl MpvFormat for bool {
    fn call_as_c_void<F : FnMut(*mut c_void)>(&self,mut f:F){
        let format = Self::get_mpv_format();
        let mut cpy : ::std::os::raw::c_int = if (*self == true) {
            1
        } else {
            0
        } ;
        let pointer = &mut cpy as *mut _ as *mut c_void;
        f(pointer)
    }

    fn get_from_c_void<F : FnMut(*mut c_void)>(mut f:F) -> bool {
        let mut temp_int = ::std::os::raw::c_int::default() ;
        let pointer = &mut temp_int as *mut _ as *mut c_void;
        f(pointer);
        match temp_int {
            0 => false,
            1 => true,
            _ => unreachable!()
        }
    }

    fn get_mpv_format() -> MpvInternalFormat {
        MpvInternalFormat::MPV_FORMAT_FLAG
    }
}

impl<'a> MpvFormat for &'a str {
    fn call_as_c_void<F : FnMut(*mut c_void)>(&self,mut f:F){
        let format = Self::get_mpv_format();
        let mut string = ffi::CString::new(*self).unwrap();
        let ptr = string.as_ptr();
        f(unsafe {mem::transmute(&ptr)})
    }

    fn get_from_c_void<F : FnMut(*mut c_void)>(mut f:F) -> &'a str {
        let char_ptr : *mut c_char = ptr::null_mut() as *mut c_char;
        f(unsafe {mem::transmute(&char_ptr)});
        unsafe {
            CStr::from_ptr(char_ptr)
                 .to_str()
                 .unwrap()
        }
    }

    fn get_mpv_format() -> MpvInternalFormat {
        MpvInternalFormat::MPV_FORMAT_STRING
    }
}

impl MpvHandler {

    /// Creates a mpv handler
    pub fn init() -> Result<MpvHandler> {
        let handle = unsafe { mpv_create() };
        if handle == ptr::null_mut() {
            return Err(MpvError::MPV_ERROR_NOMEM);
        }

        let ret = unsafe { mpv_initialize(handle) };

        ret_to_result(ret, MpvHandler { handle: handle })
    }

    pub fn get_opengl_context(&self,
                              get_proc_address: mpv_opengl_cb_get_proc_address_fn,
                              get_proc_address_ctx: *mut ::std::os::raw::c_void)
                              -> Result<OpenGLContext> {
        OpenGLContext::init(unsafe {
                                mpv_get_sub_api(self.handle,
                                                MpvSubApi::MPV_SUB_API_OPENGL_CB)
                            } as *mut mpv_opengl_cb_context,
                            get_proc_address,
                            get_proc_address_ctx)
    }

    /// Set a property synchronously
    pub fn set_property<T : MpvFormat>(&self, property: &str, value : T) -> Result<()>{
        let mut ret = 0 ;
        let format = T::get_mpv_format();
        value.call_as_c_void(|ptr:*mut c_void|{
            ret = unsafe {
                mpv_set_property(self.handle,
                                 ffi::CString::new(property).unwrap().as_ptr(),
                                 format,
                                 ptr)
            }
        });
        ret_to_result(ret,())
    }

    /// Get a property synchronously
    pub fn get_property<T : MpvFormat>(&self, property: &str) -> Result<T> {
        let mut ret = 0 ;
        let mut result : T ;
        let format = T::get_mpv_format();
        result = T::get_from_c_void(|ptr:*mut c_void|{
            ret = unsafe {
                mpv_get_property(self.handle,
                                 ffi::CString::new(property).unwrap().as_ptr(),
                                 format,
                                 ptr)
            }
        });
        ret_to_result(ret,result)
    }

    ///
    /// Set an option. Note that you can't normally set options during runtime.
    ///
    /// Changing options at runtime does not always work. For some options, attempts
    /// to change them simply fails. Many other options may require reloading the
    /// file for changes to take effect. In general, you should prefer calling
    /// mpv.set_property() to change settings during playback, because the property
    /// mechanism guarantees that changes take effect immediately.
    ///
    pub fn set_option<T : MpvFormat>(&self, property: &str, option: T) -> Result<()> {
        let mut ret = 0 ;
        let format = T::get_mpv_format();
        option.call_as_c_void(|ptr:*mut c_void|{
            ret = unsafe {
                mpv_set_option(self.handle,
                                 ffi::CString::new(property).unwrap().as_ptr(),
                                 format,
                                 ptr)
            }
        });
        ret_to_result(ret,())
    }

    /// Send a command synchronously
    pub fn command(&self, command: &[&str]) -> Result<()> {
        let command_cstring: Vec<_> = command.iter()
                                             .map(|item| ffi::CString::new(*item).unwrap())
                                             .collect();
        let mut command_pointers: Vec<_> = command_cstring.iter()
                                                          .map(|item| item.as_ptr())
                                                          .collect();
        command_pointers.push(ptr::null());

        let ret = unsafe { mpv_command(self.handle, command_pointers.as_mut_ptr()) };

        ret_to_result(ret, ())
    }

    /// Send a command asynchronously
    pub fn command_async(&self, command: &[&str], userdata :u64) -> Result<()> {
        let command_cstring: Vec<_> = command.iter()
                                             .map(|item| ffi::CString::new(*item).unwrap())
                                             .collect();
        let mut command_pointers: Vec<_> = command_cstring.iter()
                                                          .map(|item| item.as_ptr())
                                                          .collect();
        command_pointers.push(ptr::null());

        let ret = unsafe { mpv_command_async(self.handle, userdata,command_pointers.as_mut_ptr())};

        ret_to_result(ret, ())
    }

    /// Returns an Event if there is an Event available. Returns None if the event pool is empty.
    ///
    /// It is still necessary to empty the event pool even if you don't use the events
    /// Unexpected behaviour can happen
    ///
    /// # Example
    /// ```
    /// let mpv = mpv::MpvHandler::init().expect("Error while initializing MPV");
    /// while let Some(event) = mpv.wait_event(0.0) {
    ///     println!("RECEIVED EVENT : {:?}", event.event_id.to_str());
    ///     // do something else with event
    /// }
    /// ```
    ///
    ///

    pub fn wait_event(&self,timeout:f64) -> Option<Struct_mpv_event> {
        let event = unsafe {
            let ptr = mpv_wait_event(self.handle, timeout);
            if ptr.is_null() {
                panic!("Unexpected null ptr from mpv_wait_event");
            }
            *ptr
        };
        match event.event_id {
            MpvEventId::MPV_EVENT_NONE => None,
            _ => Some(event),
        }
    }
}

impl Drop for MpvHandler {
    fn drop(&mut self) {
        unsafe {
            mpv_terminate_destroy(self.handle);
        }
    }
}
