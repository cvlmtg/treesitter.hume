/* The .fini_array entry crtbeginS.o gives a GCC or Clang shared library,
   which a zig link leaves out. It runs the C++ static destructors and atexit
   handlers this library registered when the library is unloaded. Without
   it they stay registered after dlclose, and exit calls them in unmapped
   code. They are registered under &__dso_handle, as clang registers them. */
extern void *__dso_handle;
void __cxa_finalize(void *);

__attribute__((destructor)) static void hume_cxa_finalize(void) {
    __cxa_finalize(&__dso_handle);
}
