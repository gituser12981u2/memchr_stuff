use memchr_stuff::memchr_new::memchr;

fn main() {
    let arbitrary = b"elelelelelelelalal230392-302-=03-3-0w-er0w-er0w-e0w-e0w-e0w-0ew-e0w-e";

    let _ = memchr(b'y', arbitrary);
}
