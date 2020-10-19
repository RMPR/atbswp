Actually it's not really formal, but if you spot any issue, you can always submit a PR :)
# Add a new language
Create a file in the `lang` directory following the format of the language files already 
there, we also accept typo fixes.

# Install the development libraries
To be able to build atbswp, we rely on a couple of tools `poetry` for the wheel and `pyinstaller` 
to build standalone executables for the supported platforms, you can find both on PyPI.

```
pip install poetry pyinstaller
```

# Optimizations
While building the binary, you may want to enable optimizations, in that case, 
install `upx` which is easily done on Fedora:
```
sudo dnf install -y upx
```
# Known issues

## Wxpython and Linux
You may experience issues while executing `make prepare-dev` on GNU/Linux distros 
because wheels aren't available, see [this](https://wxpython.org/pages/downloads/) for 
more information. In this case, you must rely on your package manager to install globally
and copy the folder wx inside the global Python site-packages to the site-packages of the
local Python

## Workaround
I've actually removed wxPython from requirements-dev.txt now if you add a dependency you
will need to do the same. And if you are on the latest version, I removed requirements-dev 
altogether.

## Open capture, save capture and generate executable make atbswp crash
While using the releases, you can experience systematic crashes when trying to use the 
features aforementioned, this is a problem with the Glibc we built the releases against.

## Workaround
Build the executable for your architecture directly from source.
