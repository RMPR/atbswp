Actually it's not really formal, but if you spot any issue, 
you can always submit a PR :)
# Known issues
You may experience issues while executing `make prepare-dev` on GNU/Linux distros 
because wheels aren't available, see [this](https://wxpython.org/pages/downloads/) for 
more information. In this case, you must rely on your package manager to install globally
and copy the folder wx inside the global Python site-packages to the site-packages of the
local Python
