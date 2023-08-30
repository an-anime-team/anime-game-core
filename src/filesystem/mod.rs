/// Казалось бы, самый бесполезный модуль. За кой нужны эти ваши "драйверы файловой системы"
/// 
/// Дело вот в чем. У меня было сразу несколько идей реализации более продвинутых
/// хранилищ для содержимого игры. Можно, к примеру, для этого использовать OSTree и его аналоги.
/// В таком случае игра будет храниться в большом аналоге гит репозитория, можно будет откатываться
/// к любой версии, контролировать целостность содержимого и так далее. Удобно, очень.
/// 
/// Другой вариант (который будет реализован) - "многослойное хранилище". Идея заключается в том,
/// чтобы вместо того, чтобы заменять старые файлы, каждый раз создавать новый виртуальный слой -
/// отдельную папку - и загружать новые файлы туда. Далее при сборке итоговой папки с игрой мы
/// выбираем самые новые файлы из самых новых слоев. Если мы хотим откатиться к предыдущей версии -
/// мы просто отключаем лишний слой. Если нам нужны какие-то языковые пакеты - мы скачиваем их
/// в отдельные слои (которые так же создаются для новых версий пакетов) и так же накладываем их
/// друг с другом вместе со слоями с содержимым игры.
/// 
/// Для оптимизации места на диске можно реализовать сразу несколько алгоритмов. Первый - удалять
/// файлы в старых слоях если они были заменены в новом слое. Тогда пропадает возможность откатывать
/// состояния игры, но оно и не надо, в общем-то. Второй вариант - это "rebasing" слоев.
/// Фактически склейка всех слоев в один, с выбором самых новых файлов.
/// 
/// Третий вариант - можно, к примеру, написать такой драйвер, чтобы грузить разные компоненты
/// с разных дисков. Или еще откуда-то. Тут в общем воображением можно играть бесконечно.
/// 
/// Наконец, стандартный вариант - обычная папка с игрой, как это всегда и было.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::io::Result;

pub mod physical;

pub trait DriverExt: Send + Sync {
    /// Check if entry exists
    fn exists(&self, name: &OsStr) -> bool;

    /// Get entry's metadata
    fn metadata(&self, name: &OsStr) -> Result<std::fs::Metadata>;

    /// Read file content
    fn read(&self, name: &OsStr) -> Result<Vec<u8>>;

    /// Read file content as string
    fn read_to_string(&self, name: &OsStr) -> Result<String>;

    /// Read directory content
    fn read_dir(&self, name: &OsStr) -> Result<std::fs::ReadDir>;

    /// Copy file to the given path
    fn copy(&self, name: &OsStr, path: &Path) -> Result<()>;

    /// Remove file
    fn remove_file(&self, name: &OsStr) -> Result<()>;

    // TODO: create_transition must return an updater

    /// Create new transition
    /// 
    /// Transitions are needed to store intermediate downloaded data.
    /// For example, when there's a new update in the game, its `Diff`
    /// will create new transition (e.g. new folder on the disk), download
    /// all the stuff there, and then finish transition by merging this folder's
    /// content with already installed game
    /// 
    /// Concept of transitions is not useful for general approach
    /// of storing all the game's files in one folder, but is needed for alternative ones
    fn create_transition(&self, name: &str) -> Result<PathBuf>;

    /// Get transition path by name
    fn get_transition(&self, name: &str) -> Option<PathBuf>;

    /// Get list of all available transitions and their paths
    fn list_transitions(&self) -> Vec<(String, PathBuf)>;

    // TODO: finish/return transition must return an updater

    /// Finish transition
    fn finish_transition(&self, name: &str) -> Result<()>;

    /// Remove transition
    fn remove_transition(&self, name: &str) -> Result<()>;

    /// Deploy driver content to the filesystem and return path to access it
    fn deploy(&self) -> Result<PathBuf>;

    /// Dismantle deployed driver content
    fn dismantle(&self) -> Result<()>;
}

/// Get UUID from the given string
/// 
/// Needed for internal drivers work
pub fn get_uuid(text: impl AsRef<[u8]>) -> String {
    let mut uuid = [0; 16];

    for (i, byte) in text.as_ref().iter().enumerate() {
        uuid[i % 16] ^= *byte;
    }

    uuid::Builder::from_bytes(uuid)
        .into_uuid()
        .to_string()
}

/// Move files from one folder to another
pub fn move_files(from: impl AsRef<Path>, to: impl AsRef<Path>) -> Result<()> {
    for entry in from.as_ref().read_dir()?.flatten() {
        let source = entry.path();
        let target = to.as_ref().join(entry.file_name());

        if std::fs::rename(&source, &target).is_err() {
            if source.is_dir() {
                std::fs::create_dir_all(&target)
                    .and_then(|_| move_files(&source, &target))
                    .and_then(|_| std::fs::remove_dir_all(&source))?;
            }

            else if source.is_symlink() {
                std::fs::read_link(&source)
                    .and_then(|link_target| std::os::unix::fs::symlink(link_target, &target))
                    .and_then(|_| std::fs::remove_file(&source))?;
            }

            else {
                std::fs::copy(&source, &target)
                    .and_then(|_| std::fs::remove_file(&source))?;
            } 
        }
    }

    Ok(())
}
