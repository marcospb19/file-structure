use crate::{File, FileType};

use std::{collections::VecDeque, path::PathBuf};

#[derive(Debug, Clone)]
pub struct FilesIter<'a> {
    // Directories go at the back, files at the front
    // Has a aditional field for keeping track of depth
    pub(crate) file_deque: VecDeque<(&'a File, usize)>,
    // Options
    pub(crate) files_before_directories: bool,
    pub(crate) skip_dirs: bool,
    pub(crate) skip_regular_files: bool,
    pub(crate) skip_symlinks: bool,
    pub(crate) min_depth: usize,
    pub(crate) max_depth: usize,
}

impl<'a> FilesIter<'a> {
    pub fn paths(self) -> PathsIter<'a> {
        PathsIter {
            file_iter: self,
            current_path: PathBuf::new(),
            show_full_relative_path: true,
        }
    }

    // -- from here, only filters --
    pub fn files_before_directories(mut self, arg: bool) -> Self {
        self.files_before_directories = arg;
        self
    }

    pub fn skip_dirs(mut self, arg: bool) -> Self {
        self.skip_dirs = arg;
        self
    }

    pub fn skip_regular_files(mut self, arg: bool) -> Self {
        self.skip_regular_files = arg;
        self
    }

    pub fn skip_symlinks(mut self, arg: bool) -> Self {
        self.skip_symlinks = arg;
        self
    }

    pub fn min_depth(mut self, min: usize) -> Self {
        self.min_depth = min;
        self
    }

    pub fn max_depth(mut self, max: usize) -> Self {
        self.max_depth = max;
        self
    }

    // The only way to implement Default without exposing to public API:
    pub(crate) fn default() -> Self {
        FilesIter {
            file_deque: VecDeque::new(),
            // Options
            files_before_directories: false,
            skip_dirs: false,
            skip_regular_files: false,
            skip_symlinks: false,
            min_depth: usize::MIN,
            max_depth: usize::MAX,
        }
    }
}

impl<'a> Iterator for FilesIter<'a> {
    type Item = &'a File;

    fn next(&mut self) -> Option<Self::Item> {
        if self.file_deque.is_empty() {
            return None;
        }

        // Pop from left or right?
        //
        // If self.files_before_directories is set, always pop from the left, where
        // files are located
        //
        // Else, pop files from the right, that are directories, until there's no
        // directory left, then start popping from the left
        //
        // Note: last_file_is_directory <-> there is a directory in the deque
        let last_file_is_directory = self.file_deque.back().unwrap().0.file_type.is_dir();
        let pop_from_the_left = self.files_before_directories || !last_file_is_directory;

        // Unpack popped file and depth
        let (file, depth) = if pop_from_the_left {
            self.file_deque.pop_front()
        } else {
            self.file_deque.pop_back()
        }
        .unwrap();

        // If directory, add children, and check for `self.skip_dirs`
        if let FileType::Directory(ref children) = &file.file_type {
            // Reversed, because late nodes stay at the tip
            // We want at the tip the first ones
            for child in children.iter().rev() {
                if child.file_type.is_dir() {
                    self.file_deque.push_back((child, depth + 1));
                } else {
                    self.file_deque.push_front((child, depth + 1));
                }
            }
        }

        // If should skip due to depth limits
        if self.min_depth > depth || self.max_depth < depth {
            return self.next();
        }

        // If should skip due file type specific skip filters
        if self.skip_regular_files && file.file_type.is_regular()
            || self.skip_dirs && file.file_type.is_dir()
            || self.skip_dirs && file.file_type.is_dir()
        {
            return self.next();
        }

        Some(&file)
    }
}

#[derive(Debug, Clone)]
pub struct PathsIter<'a> {
    // We will make a lot of pushs and pops in this path from each segment of path
    current_path: PathBuf,
    file_iter: FilesIter<'a>,
    // options
    show_full_relative_path: bool,
}

impl PathsIter<'_> {
    pub fn show_full_relative_path(mut self, arg: bool) -> Self {
        self.show_full_relative_path = arg;
        self
    }
}

impl<'a> Iterator for PathsIter<'a> {
    type Item = &'a PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.file_iter.next()?;

        if self.show_full_relative_path {
            Some(&next.path)
        } else {
            // Some(&next.path)
            None
        }
    }
}

#[cfg(test)]
mod tests {
    #[test] // Huge test ahead
    #[rustfmt::skip]
    fn testing_files_and_paths_iters() {
        use crate::{File, FileType::*};

        // Implementing a syntax sugar util to make tests readable
        impl File {
            fn c(&self, index: usize) -> &File {
                &self.file_type.children().unwrap()[index]
            }
        }

        // We will test the following structure:
        // ".config": [
        //     "i3": [
        //         "file1"
        //         "file2"
        //         "dir": [
        //             "innerfile1"
        //             "innerfile2"
        //         ]
        //         "file3"
        //     ]
        //     "outerfile1"
        //     "outerfile2"
        // ]

        // Create the strucutre
        #[rustfmt::skip]
        let root = File::new(".config", Directory(vec![
            File::new("i3", Directory(vec![
                File::new("file1", Regular),
                File::new("file2", Regular),
                File::new("dir", Directory(vec![
                    File::new("innerfile1", Regular),
                    File::new("innerfile2", Regular)
                ])),
                File::new("file3", Regular),
            ])),
            File::new("outerfile1", Regular),
            File::new("outerfile2", Regular)
        ]));

        #[rustfmt::skip]
        // Get the references in line order, from top to bottom
        let refs = vec![
            /* 0 */ &root,                // .config/
            /* 1 */ &root.c(0),           // .config/i3/
            /* 2 */ &root.c(0).c(0),      // .config/i3/file1
            /* 3 */ &root.c(0).c(1),      // .config/i3/file2
            /* 4 */ &root.c(0).c(2),      // .config/i3/dir/
            /* 5 */ &root.c(0).c(2).c(0), // .config/i3/dir/innerfile1
            /* 6 */ &root.c(0).c(2).c(1), // .config/i3/dir/innerfile2
            /* 7 */ &root.c(0).c(3),      // .config/i3/file3
            /* 8 */ &root.c(1),           // .config/outerfile1
            /* 9 */ &root.c(2),           // .config/outerfile2
        ];

        let mut it = root.files();
        assert_eq!(it.next(), Some(refs[0])); // .config/
        assert_eq!(it.next(), Some(refs[1])); // .config/i3/
        assert_eq!(it.next(), Some(refs[4])); // .config/i3/dir/
        assert_eq!(it.next(), Some(refs[5])); // .config/i3/dir/innerfile1
        assert_eq!(it.next(), Some(refs[6])); // .config/i3/dir/innerfile2
        assert_eq!(it.next(), Some(refs[2])); // .config/i3/file1
        assert_eq!(it.next(), Some(refs[3])); // .config/i3/file2
        assert_eq!(it.next(), Some(refs[7])); // .config/i3/file3
        assert_eq!(it.next(), Some(refs[8])); // .config/outerfile1
        assert_eq!(it.next(), Some(refs[9])); // .config/outerfile2

        let mut it = root.files().files_before_directories(true);
        assert_eq!(it.next(), Some(refs[0])); // .config/
        assert_eq!(it.next(), Some(refs[8])); // .config/outerfile1
        assert_eq!(it.next(), Some(refs[9])); // .config/outerfile2
        assert_eq!(it.next(), Some(refs[1])); // .config/i3/
        assert_eq!(it.next(), Some(refs[2])); // .config/i3/file1
        assert_eq!(it.next(), Some(refs[3])); // .config/i3/file2
        assert_eq!(it.next(), Some(refs[7])); // .config/i3/file3
        assert_eq!(it.next(), Some(refs[4])); // .config/i3/dir/
        assert_eq!(it.next(), Some(refs[5])); // .config/i3/dir/innerfile1
        assert_eq!(it.next(), Some(refs[6])); // .config/i3/dir/innerfile2

        let mut it = root.files().skip_dirs(true);
        assert_eq!(it.next(), Some(refs[5])); // .config/i3/dir/innerfile1
        assert_eq!(it.next(), Some(refs[6])); // .config/i3/dir/innerfile2
        assert_eq!(it.next(), Some(refs[2])); // .config/i3/file1
        assert_eq!(it.next(), Some(refs[3])); // .config/i3/file2
        assert_eq!(it.next(), Some(refs[7])); // .config/i3/file3
        assert_eq!(it.next(), Some(refs[8])); // .config/outerfile1
        assert_eq!(it.next(), Some(refs[9])); // .config/outerfile2

        let mut it = root.files().skip_regular_files(true);
        assert_eq!(it.next(), Some(refs[0])); // .config/
        assert_eq!(it.next(), Some(refs[1])); // .config/i3/
        assert_eq!(it.next(), Some(refs[4])); // .config/i3/dir/

        // min and max depth (1 <= d <= 2)
        //
        // skips:
        // .config/
        // .config/i3/dir/innerfile1
        // .config/i3/dir/innerfile2
        let mut it = root.files().min_depth(1).max_depth(2);
        assert_eq!(it.next(), Some(refs[1])); // .config/i3/
        assert_eq!(it.next(), Some(refs[4])); // .config/i3/dir/
        assert_eq!(it.next(), Some(refs[2])); // .config/i3/file1
        assert_eq!(it.next(), Some(refs[3])); // .config/i3/file2
        assert_eq!(it.next(), Some(refs[7])); // .config/i3/file3
        assert_eq!(it.next(), Some(refs[8])); // .config/outerfile1
        assert_eq!(it.next(), Some(refs[9])); // .config/outerfile2

        // ---------------------
        //
        // Paths iterator testing
        let mut it = root.paths();
        assert_eq!(it.next(), Some(&refs[0].path)); // ".config/"
        assert_eq!(it.next(), Some(&refs[1].path)); // ".config/i3/"
        assert_eq!(it.next(), Some(&refs[4].path)); // ".config/i3/dir/"
        assert_eq!(it.next(), Some(&refs[5].path)); // ".config/i3/dir/innerfile1"
        assert_eq!(it.next(), Some(&refs[6].path)); // ".config/i3/dir/innerfile2"
        assert_eq!(it.next(), Some(&refs[2].path)); // ".config/i3/file1"
        assert_eq!(it.next(), Some(&refs[3].path)); // ".config/i3/file2"
        assert_eq!(it.next(), Some(&refs[7].path)); // ".config/i3/file3"
        assert_eq!(it.next(), Some(&refs[8].path)); // ".config/outerfile1"
        assert_eq!(it.next(), Some(&refs[9].path)); // ".config/outerfile2"
    }
}
