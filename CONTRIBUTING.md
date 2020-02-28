Actually it's not really formal, but if you spot any issue, you can always submit a PR :)
# Optimizations
To enable optimizations, you may want to install `upx` which is easily done on Fedora:
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

### Workaround
I've actually removed wxPython from requirements-dev.txt now if you add a dependency you
will need to do the same.
