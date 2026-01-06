#[cfg(test)]
mod tests {
    #![allow(unused_imports)]
    use crate::memchr_new::memrchr;
    use std::env::temp_dir;
    use std::ffi::OsStr;
    use std::ffi::OsString;
    use std::fs;
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::ffi::OsStrExt;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    const PATHS: &[&[u8]] = &[
    b"/proc/1289/task/1423/clear_refs",
    b"/home/alexc/llvm-project/clang/test/CodeGen/shared-string-literals.c",
    b"/var/lib/pacman/local/perl-path-tiny-0.150-2/desc",
    b"/proc/1236/task/1236/fdinfo/8",
    b"/usr/lib/python3.13/site-packages/tzdata/zoneinfo/Etc/GMT-13",
    b"/proc/2637794/map_files/7f1f6f295000-7f1f6f2bf000",
    b"/proc/2643351/task/2643361/net/psched",
    b"/tmp/llvm-project/llvm/test/CodeGen/ARM/ssat.ll",
    b"/usr/lib32/libwebpdemux.so",
    b"/tmp/.hidden_file.txt",
    b"/var/lib/flatpak/runtime/org.freedesktop.Platform/x86_64/24.08/a993292d6ff150598dad4cd1f725aeee01a668b9e721b559ea1b6f6240174d58/files/share/zoneinfo/Pacific/Kosrae",
    b"/home/alexc/llvm-project/libc/test/src/math/f16sqrtl_test.cpp",
    b"/usr/lib/node_modules/yarn/node_modules/rxjs/src/internal/Observer.ts",
    b"/usr/share/locale/zh_TW/LC_MESSAGES/plasma_runner_konsoleprofiles.mo",
    b"/home/alexc/Farrier-app/node_modules/@react-native/gradle-plugin/package.json",
    b"/usr/lib/racket/compiled/usr/share/racket/pkgs/draw-doc/scribblings/draw/compiled/radial-gradient-class_scrbl.dep",
    b"/usr/share/icons/breeze-dark/places/24/folder-image-symbolic.svg",
    b"/usr/share/icons/breeze-dark/actions/16/find-location-symbolic.svg",
    b"/usr/share/texmf-dist/fonts/vf/public/montserrat/Montserrat-ExtraLight-tosf-sc-ot1.vf",
    b"/proc/2659810/task/2659812/net/fib_trie",
    b"/home/alexc/llvm-project/lldb/source/Breakpoint/BreakpointID.cpp",
    b"/proc/1612/task/1614/stat",
    b"/usr/include/glm/gtx/extended_min_max.hpp",
    b"/usr/include/qt/QtWidgets/qgraphicssceneevent.h",
    b"/proc/2594422/task/2594651/pagemap",
    b"/tmp/llvm-project/llvm/lib/Transforms/Utils/LibCallsShrinkWrap.cpp",
    b"/proc/2637531/task/2637594/net/mcfilter6",
    b"/usr/share/calligra/stencils/Cisco/running_man_subdued.png",
    b"/tmp/llvm-project/libcxx/test/std/numerics/numarray/valarray.nonmembers/valarray.binary/shift_left_valarray_valarray.pass.cpp",
    b"/usr/share/texmf-dist/fonts/tfm/public/nunito/Nunito-Italic-sup-t1.tfm",
    b"/home/alexc/embedded/rp-pico2w-examples/target/thumbv8m.main-none-eabihf/doc/embassy_rp/rom_data/reboot/sidebar-items.js",
    b"/usr/share/texmf-dist/tex/latex/utfsym/usym1F038.tikz",
    b"/usr/share/ri/3.4.0/system/Zlib/gunzip-c.ri",
    b"/proc/2638048/task/2638116/smaps",
    b"/proc/31/task/31/schedstat",
    b"/usr/lib/qt6/qml/QtQuick/Controls/FluentWinUI3/dark/images/radiobutton-indicator-checked-pressed.png",
    b"/var/lib/aurbuild/x86_64/root/usr/share/i18n/locales/my_MM",
    b"/usr/lib/ruby/gems/3.4.0/doc/rubygems-3.6.9/ri/Gem/latest_spec_for-c.ri",
    b"/home/alexc/llvm-project/clang/test/Driver/mingw-auto-import.c",
    b"/home/alexc/llvm-project/llvm/test/tools/llvm-objdump/help.test",
    b"/usr/lib/node_modules/@google/gemini-cli/node_modules/@opentelemetry/resources/build/esnext/detectors/platform/browser/ProcessDetector.js.map",
    b"/usr/lib/node_modules/pyright/dist/typeshed-fallback/stubs/yt-dlp/yt_dlp/compat/compat_utils.pyi",
    b"/usr/lib/gcc/aarch64-linux-gnu/15.1.0/plugin/include/selftest-rtl.h",
    b"/usr/include/gtkmm-4.0/gdkmm/cairocontext.h",
    b"/opt/azure-cli/lib/python3.13/site-packages/azure/mgmt/resource/changes/__pycache__/models.cpython-313.pyc",
    b"/proc/2739/map_files/7f631de00000-7f631dedc000",
    b"/opt/azure-cli/lib/python3.13/site-packages/azure/mgmt/web/v2022_09_01/aio/operations/__pycache__/_static_sites_operations.cpython-313.pyc",
    b"/proc/1597/net/rt_cache",
    b"/proc/2682450/task/2682451/fdinfo/0",
    b"/tmp/llvm-project/clang/test/CodeGen/mcount.c",
    b"/usr/lib/go/src/cmd/go/internal/modfetch/toolchain.go",
];

    fn test_memchr(search: u8, sl: &[u8]) {
        let memchrtest = crate::memchr_new::memchr(search, sl);
        let realans = sl.iter().position(|b| *b == search);
        let as_array = &[search];
        let char = String::from_utf8_lossy(as_array);
        assert!(
            memchrtest == realans,
            "test failed for {} in memrchr real ans is {realans:?} {memchrtest:?} for char {char} ",
            String::from_utf8_lossy(sl)
        );
    }

    fn test_memrchr(search: u8, sl: &[u8]) {
        let realans = sl.iter().rposition(|b| *b == search);
        let memrchrtest = memrchr(search, sl);
        if realans != memrchrtest {
            let as_array = &[search];
            let char = String::from_utf8_lossy(as_array);
            let search_len = realans.unwrap();

            let slice = String::from_utf8_lossy(&sl[search_len..search_len + 8]);

            assert!(
                memrchrtest == realans,
                "test failed for {} in memrchr real ans is {realans:?} {memrchrtest:?} for character {char} with ascii value {search}

                showing slice  as '{slice}'
                ",
                String::from_utf8_lossy(sl)

            );
        }
    }

    #[test]
    fn tmemchr() {
        let random_chars = 0..u8::MAX;

        for items in random_chars {
            for paths in PATHS {
                test_memchr(items, *paths);
            }
        }
    }

    #[test]
    fn tmemrchr() {
        let random_chars = 0..u8::MAX;

        for items in random_chars {
            for paths in PATHS {
                test_memrchr(items, *paths)
            }
        }
    }
}
