# Package

version       = "0.1.0"
author        = "SolracHQ"
description   = "A simple Fantasy VM"
license       = "MIT"
srcDir        = "src"
installExt    = @["nim"]
bin           = @["fvm"]


# Dependencies

requires "nim >= 2.2.6"

requires "cligen >= 1.9.6"