# Atbswp
Literally Automate the boring stuff with python, allows the user to record his mouse and keyboard 
actions and reproduce them identically as many times as he wants.

# Install instructions
Fedora
```shell
git clone https://github.com/RMPR/atbswp.git && cd atbswp

sudo dnf install python3-wxpython4

python -m venv .

make prepare-dev

make run
```
Debian
```shell
git clone https://github.com/RMPR/atbswp.git && cd atbswp

sudo apt install python3-wxgtk4.0

python -m venv .

make prepare-dev

make run
```
Windows
```shell
git clone https://github.com/rmpr/atbswp

cd atbswp\

pip install wxPython 

pip install -r requirements-dev.txt

python atbswp\atbswp.py
```

# Demo

![atbswp quick demo](demo/demo.gif)

# Contributions
See [CONTRIBUTING.md](./CONTRIBUTING.md)
