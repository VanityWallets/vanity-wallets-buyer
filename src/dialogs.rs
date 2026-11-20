use std::path::PathBuf;

pub struct FileDialog {
    pub save: bool,
    pub title: String,
    pub file_name: String,
    pub start_dir: Option<PathBuf>,
    pub filter: String,
    pub owner: isize,
}

impl FileDialog {
    pub fn spawn(self, done: impl FnOnce(Option<PathBuf>) + Send + 'static) {
        std::thread::spawn(move || done(show(&self)));
    }
}

#[cfg(windows)]
fn show(d: &FileDialog) -> Option<PathBuf> {
    use windows::core::{HSTRING, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, IBindCtx,
        CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::Common::COMDLG_FILTERSPEC;
    use windows::Win32::UI::Shell::{
        FileOpenDialog, FileSaveDialog, IFileDialog, IFileOpenDialog, IFileSaveDialog, IShellItem,
        SHCreateItemFromParsingName, FILEOPENDIALOGOPTIONS, FOS_FILEMUSTEXIST, FOS_FORCEFILESYSTEM,
        FOS_NOREADONLYRETURN, FOS_OVERWRITEPROMPT, FOS_PATHMUSTEXIST, SIGDN_FILESYSPATH,
    };
    use windows::core::Interface;

    let filter_name = HSTRING::from(d.filter.as_str());
    let filter_spec = HSTRING::from("*.json");
    let specs = [COMDLG_FILTERSPEC { pszName: PCWSTR(filter_name.as_ptr()), pszSpec: PCWSTR(filter_spec.as_ptr()) }];

    unsafe {
        let init = CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE);
        let picked = (|| -> windows::core::Result<Option<PathBuf>> {
            let dialog: IFileDialog = if d.save {
                let s: IFileSaveDialog = CoCreateInstance(&FileSaveDialog, None, CLSCTX_INPROC_SERVER)?;
                s.cast()?
            } else {
                let o: IFileOpenDialog = CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)?;
                o.cast()?
            };
            let mut options = dialog.GetOptions()?.0 | FOS_FORCEFILESYSTEM.0 | FOS_PATHMUSTEXIST.0;
            if d.save {
                options = (options | FOS_NOREADONLYRETURN.0) & !FOS_OVERWRITEPROMPT.0;
            } else {
                options |= FOS_FILEMUSTEXIST.0;
            }
            dialog.SetOptions(FILEOPENDIALOGOPTIONS(options))?;
            dialog.SetTitle(&HSTRING::from(d.title.as_str()))?;
            dialog.SetFileTypes(&specs)?;
            dialog.SetDefaultExtension(&HSTRING::from("json"))?;
            if d.save {
                dialog.SetFileName(&HSTRING::from(d.file_name.as_str()))?;
            }
            if let Some(dir) = d.start_dir.as_deref().filter(|p| p.is_dir()) {
                if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(
                    &HSTRING::from(dir.as_os_str()),
                    None::<&IBindCtx>,
                ) {
                    let _ = dialog.SetFolder(&item);
                }
            }
            if dialog.Show(HWND(d.owner as *mut core::ffi::c_void)).is_err() {
                return Ok(None);
            }
            let name = dialog.GetResult()?.GetDisplayName(SIGDN_FILESYSPATH)?;
            let path = name.to_string();
            CoTaskMemFree(Some(name.0 as *const _));
            Ok(path.ok().map(PathBuf::from))
        })();
        if init.is_ok() {
            CoUninitialize();
        }
        picked.ok().flatten()
    }
}

#[cfg(not(windows))]
fn show(_d: &FileDialog) -> Option<PathBuf> {
    None
}
