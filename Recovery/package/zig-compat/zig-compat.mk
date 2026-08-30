# Zig does not emit CMake's mutable RPATH padding.  Target libraries already
# live in /usr/lib, so no embedded runtime path is required.
JPEG_TURBO_CONF_OPTS += -DCMAKE_SKIP_RPATH=ON

# gesftpserver uses Python only for its test harness.  Its old configure
# probe predates the python3 executable name.
GESFTPSERVER_CONF_ENV += rjk_cv_python24=python3
