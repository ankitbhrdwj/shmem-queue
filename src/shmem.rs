use alloc::format;
use cstr_core::CString;
use libc::{c_char, c_int, c_void, mode_t};
use libc::{ftruncate, mmap, munmap, open as file_open, unlink};
use libc::{
    MAP_FAILED, MAP_SHARED, O_CLOEXEC, O_CREAT, O_NOFOLLOW, O_RDWR, PROT_READ, PROT_WRITE, S_IRUSR,
    S_IWUSR,
};

static mut NAME: Option<CString> = None;

unsafe fn shm_open(name: CString, oflag: c_int, mode: mode_t) -> c_int {
    let name = CString::new(format!("b/dev/shm/{:?}", name)).unwrap();
    let oflag = oflag | O_NOFOLLOW | O_CLOEXEC;

    let ret = file_open(name.as_ptr(), oflag, mode);
    if ret < 0 {
        log::error!("shm_open failed");
        return -1;
    }
    ret
}

unsafe fn shm_unlink(name: *const c_char) -> c_int {
    let result = unlink(name);
    if result < 0 {
        log::error!("shm_unlink failed");
        return -1;
    }
    result
}

/// Open shared memory object with given name and size.
///
/// # Arguments
/// * `name` - name of the shared memory object
/// * `size` - size of the shared memory object
///
/// # Returns
/// * `SharedMemory` - shared memory object
fn open(name: &str, size: usize) -> i32 {
    let c_name = CString::new(name).unwrap();
    unsafe {
        NAME = Some(c_name.clone());
    }
    let shm_fd = unsafe { shm_open(c_name, O_CREAT | O_RDWR, S_IRUSR | S_IWUSR) };
    if shm_fd < 0 {
        log::error!("shm_open failed");
        return -1;
    }

    let shm_size = unsafe { ftruncate(shm_fd, size as i64) };
    if shm_size < 0 {
        log::error!("ftruncate failed");
        return -1;
    }

    shm_fd
}

/// Mmap shared memory object with given file descriptor.
///
/// # Arguments
/// * `fd` - file descriptor
/// * `size` - size of the shared memory object
///
/// # Returns
/// * pointer to the shared memory object
fn map(fd: i32, size: usize) -> *mut c_void {
    let ptr = unsafe {
        mmap(
            core::ptr::null_mut(),
            size,
            PROT_READ | PROT_WRITE,
            MAP_SHARED,
            fd,
            0,
        )
    };

    if ptr == MAP_FAILED {
        log::error!("mmap failed");
        return core::ptr::null_mut();
    }

    ptr
}

/// Create a new shared memory object with given name and size.
///
/// # Arguments
/// * `name` - name of the shared memory object
/// * `size` - size of the shared memory object
///
/// # Returns
/// * `ptr` - pointer to the shared memory object
pub fn create_shm(name: &str, size: usize) -> *mut c_void {
    let fd = open(name, size);

    map(fd, size)
}

/// Remove the shared memory object with given name.
///
/// # Arguments
/// * `name` - name of the shared memory object
pub fn unlink_shm(ptr: *mut c_void, size: usize) {
    unsafe {
        if NAME.is_some() {
            let c_name = NAME.as_ref().unwrap();
            shm_unlink(c_name.as_ptr());
            munmap(ptr, size);
        }
    }
}
