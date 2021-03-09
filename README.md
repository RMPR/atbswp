# Atbswp
![Logo](./atbswp/img/icon.png)

Literally Automate the boring stuff with Python, allows the user to record his mouse and keyboard 
actions and reproduce them identically as many times as he wants.


# Install instructions

## Download

You can download and run the installer/archive for your operating system (currently Windows and Linux) 
from [here](https://github.com/rmpr/atbswp/releases)

## From source

Fedora
```shell
git clone https://github.com/RMPR/atbswp.git && cd atbswp
sudo dnf install python3-wxpython4
make prepare-dev
make run
```
Debian
```shell
sudo apt install git python3-dev python3-tk python3-setuptools python3-wheel python3-pip python3-wxgtk4.0
git clone https://github.com/RMPR/atbswp.git && cd atbswp
python3 -m pip install pyautogui pynput --user
python3 atbswp/atbswp.py
```
Manjaro/Arch
```shell
sudo pacman -S tk python-wxpython
python3 -m pip install pyautogui pynput --user
git clone https://github.com/RMPR/atbswp.git && cd atbswp
python3 atbswp/atbswp.py
```
Windows
```shell
git clone https://github.com/rmpr/atbswp
cd atbswp\
pip install wxPython pyautogui pynput
python atbswp\atbswp.py
```

# Demo

![atbswp quick demo](demo/demo.gif)

# Use cases
I've mainly used it to automate gold/point/XP farming in games, I think this can also be used to:

- Automate a demo during a conference for example
- Automate UAT in the devops process (as long as you're making something with a GUI).

If you use it for something really cool you can always reach me at github (at) rmpr (dot) xyz or drop
a PR :). Bonus points if you have a demo video.

# Donate
<a href="https://www.buymeacoffee.com/rmpr" target="_blank"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-black.png" width="150" alt="Buy me a coffee"></a>

If you found this helpful in any way.


# Contributions
Contributions are welcomed, see [CONTRIBUTING.md](./CONTRIBUTING.md)

# Known issues
On Linux, this only works with Xorg, with wayland support coming soon, for now you have to
enable Xorg. 

```
sudo sed 's/#WaylandEnable=false/WaylandEnable=false/' /etc/gdm/custom.conf -i # on Gnome
```

# Join us
To keep up with the latest news about atbswp you can reach us on this [telegram channel](https://t.me/atbswp) we will
post important news and periodically runs polls to keep the users feedback.
