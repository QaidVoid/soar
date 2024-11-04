
## [0.4.0](https://github.com/QaidVoid/soar/compare/v0.3.1..v0.4.0) - 2024-11-04

### ⛰️  Features

- *(download)* Add progressbar & output file path support - ([f7dcea8](https://github.com/QaidVoid/soar/commit/f7dcea8ef6a19e3a8496c78d1ea9097846ecff28))
- *(download)* Fallback to download package if invalid URL - ([eccbb87](https://github.com/QaidVoid/soar/commit/eccbb87e640af2477e3c55fe41c0e344f6b25da0))
- *(flatimage)* Integrate flatimage using remote files - ([e94d480](https://github.com/QaidVoid/soar/commit/e94d48085fb2e64f61b09053d0c6578d2e7761cb))
- *(inspect)* Add inspect command to view build script - ([bcef36c](https://github.com/QaidVoid/soar/commit/bcef36cbc0045230357ca37afb5c7480f4cab046))
- *(progress)* Re-implement installation progress bar - ([89ed804](https://github.com/QaidVoid/soar/commit/89ed804e396944b4e53a8091c0024e261509add5))
- *(yes)* Skip prompts and select first value - ([286743e](https://github.com/QaidVoid/soar/commit/286743e60c900a915fd6821ff47e13a66ceaf234))

### 🐛 Bug Fixes

- *(download)* Don't hold downloads in memory - ([baf33d9](https://github.com/QaidVoid/soar/commit/baf33d997a8f2a75d965094aa129ad44348fc194))
- *(health)* Check fusermount3 and use fusermount as fallback - ([3cef007](https://github.com/QaidVoid/soar/commit/3cef007d12351c2226f1006961795b7a6a4f4ed8))
- *(image)* Fix image rendering - ([b190bd0](https://github.com/QaidVoid/soar/commit/b190bd0eaa09fd2357939fd0986e62d94fcfcb4a))
- *(package)* Fix multi-repo install handling - ([8654fbb](https://github.com/QaidVoid/soar/commit/8654fbbc4c84c7f632f9e971732f60b960c01fd9))
- *(remove)* Improve package removal - ([3f0307a](https://github.com/QaidVoid/soar/commit/3f0307aab929ed83e2f602cf33763162095cd343))
- *(update)* Fix update progressbar - ([948a42e](https://github.com/QaidVoid/soar/commit/948a42eab471a6dde413636ba0b8c0933e7d47c0))

### 🚜 Refactor

- *(health)* Separate user namespaces and fuse issues - ([4b7fd4f](https://github.com/QaidVoid/soar/commit/4b7fd4f9219ce93a8b7612b38f1d68cf38b5ee0d))
- *(image)* Reduce image handling complexity - ([39e9c1b](https://github.com/QaidVoid/soar/commit/39e9c1b3e97a6c628abe5d092adafba37ff30b9d))
- *(list)* Sort list output - ([2c8d894](https://github.com/QaidVoid/soar/commit/2c8d8945ad80d4578d815b72b5791fd111257f26))
- *(project)* Minor refactor - ([0b0bd06](https://github.com/QaidVoid/soar/commit/0b0bd06811fbe3d7a91d6e46a5b2598a4ffe5957))

### 📚 Documentation

- *(README)* Fix installation instructions - ([b2fc746](https://github.com/QaidVoid/soar/commit/b2fc74664da9463a82d1f445d1560c28d7134f66))
- *(readme)* Update README - ([2fb53cc](https://github.com/QaidVoid/soar/commit/2fb53cc42378d17c64388a7b780298ab82de103e))

### ⚙️ Miscellaneous Tasks

- *(script)* Update install script - ([a18cba3](https://github.com/QaidVoid/soar/commit/a18cba3092c892173d00551796d1b8c489cf8324))
- *(script)* Add install script - ([7bea339](https://github.com/QaidVoid/soar/commit/7bea3393b1d9f6ada476b9f3b55b875051ef8f6f))
- *(workflow)* Remove existing nightly before publishing new - ([e1171af](https://github.com/QaidVoid/soar/commit/e1171af85b6816c512cdf1ab91c01580ba5195a8))


## [0.3.1](https://github.com/QaidVoid/soar/compare/v0.3.0..v0.3.1) - 2024-10-26

### 🐛 Bug Fixes

- *(config)* Fix default config url - ([1862a7e](https://github.com/QaidVoid/soar/commit/1862a7eb7ca6106bd3834ec6cf24a85e9e09ccc3))


## [0.3.0](https://github.com/QaidVoid/soar/compare/v0.2.0..v0.3.0) - 2024-10-26

### ⛰️  Features

- *(appimage)* Allow providing portable home/config dir for appimage - ([446958e](https://github.com/QaidVoid/soar/commit/446958e3a57a58c0a42de3f2103f6f7995a791cf))
- *(appimage)* Implement appimage integration - ([3d7fbe1](https://github.com/QaidVoid/soar/commit/3d7fbe198e53c1e0b3d88e48d7f917e0f0c6ee30))
- *(collection)* Allow dynamic collection names - ([d37bad0](https://github.com/QaidVoid/soar/commit/d37bad073642e04276140c3e40d85399fa9a86c5))
- *(color)* Implement colorful logging - ([61d9ceb](https://github.com/QaidVoid/soar/commit/61d9ceb1f39c43fa86cc2da8ab8292e4ffa2ec70))
- *(health)* Include fuse check - ([ee9d3b7](https://github.com/QaidVoid/soar/commit/ee9d3b7984ce67c13f712d7efc22c3619b18903e))
- *(health)* Add health check command - ([293960f](https://github.com/QaidVoid/soar/commit/293960fa9eb5365a34d5794ef8889ff111087aac))
- *(image)* Add halfblock image support - ([a1e2dc3](https://github.com/QaidVoid/soar/commit/a1e2dc37d5b9b30f76e7e8c59a4126afe517b58f))
- *(image)* Add sixel support - ([88433d3](https://github.com/QaidVoid/soar/commit/88433d3c2b399f4269b4885514b88b1ca7c5a14b))
- *(image)* Kitty graphics protocol image support for query - ([fb1da68](https://github.com/QaidVoid/soar/commit/fb1da6891f1dfcf24ef2f9ad50d7cba68d3b0b87))
- *(pkg)* Fetch remote image/desktop file if pkg is not appimage - ([2e5b15e](https://github.com/QaidVoid/soar/commit/2e5b15e1622d60f99d1e29a5885cbf0f31691a84))

### 🐛 Bug Fixes

- *(appimage)* Sanity checks for kernel features & user namespace - ([b8dd511](https://github.com/QaidVoid/soar/commit/b8dd511d2425848b2f479660ce9349c7ec90a243))
- *(appimage)* Prevent creating portable dirs by default - ([cc66cd3](https://github.com/QaidVoid/soar/commit/cc66cd3580eb4b8d039ac09c2ae279f3c1c1ba26))
- *(appimage)* Set default portable path if arg is not provided - ([5a34205](https://github.com/QaidVoid/soar/commit/5a34205d6e2016cd336021f520dae6b0996810a7))
- *(appimage)* Use path check for ownership - ([7181629](https://github.com/QaidVoid/soar/commit/7181629ad4b94c7bcefa3d50348f3964be80aae7))
- *(appimage)* Handle symlinks and use proper icon path - ([aee9282](https://github.com/QaidVoid/soar/commit/aee92820469db7a39aea30c5cc1fca56ba7a8e05))
- *(fetch)* Fetch default icons only when fetcher is called - ([fdefcd5](https://github.com/QaidVoid/soar/commit/fdefcd59d54fe3357f0c096cca26d1fdedf27001))
- *(image)* Fetch default fallback image - ([bc92204](https://github.com/QaidVoid/soar/commit/bc9220451e2f22d6fba8761d487afee4485f2fd1))
- *(registry)* Update outdated local registry - ([6a967df](https://github.com/QaidVoid/soar/commit/6a967df7a249e1ebb42a61cbec661908d0b2343d))
- *(userns-check)* Check clone_newuser support - ([2e1cf13](https://github.com/QaidVoid/soar/commit/2e1cf1332af9a858482ddd48cea035d0e8ead98c))
- *(wrap)* Fix text wrapping - ([e7b6d71](https://github.com/QaidVoid/soar/commit/e7b6d71e38720ad95bf4914fe63e6395b0d8f0ab))

### 🚜 Refactor

- *(collection)* Rename root_path to collection - ([a480c85](https://github.com/QaidVoid/soar/commit/a480c8581a7531ed9b8c94ebedf16975c4bdaf63))
- *(color)* Update colors in query - ([adc257b](https://github.com/QaidVoid/soar/commit/adc257bf8235b17512eae113d8f96a5916aa1e6a))
- *(package)* Reduce hard-coded collections - ([041e824](https://github.com/QaidVoid/soar/commit/041e824fca58e3c2c24f5417e1a7a772ce563746))

### ⚙️ Miscellaneous Tasks

- *(readme)* Update Readme - ([8f43a68](https://github.com/QaidVoid/soar/commit/8f43a6843e73530dcca086591831bb0c415f78a0))
- *(workflow)* Run nightly on every commit - ([42ddf90](https://github.com/QaidVoid/soar/commit/42ddf90857a1c9a0ff264dbac45e1fda114c0935))
- *(workflow)* Add nightly workflow - ([f697a5f](https://github.com/QaidVoid/soar/commit/f697a5f86adc4c75822e0c8fc3b3a0e7dacd9479))

## New Contributors ❤️

* @dependabot[bot] made their first contribution in [#1](https://github.com/QaidVoid/soar/pull/1)

## [0.2.0](https://github.com/QaidVoid/soar/compare/v0.1.0..v0.2.0) - 2024-10-11

### ⛰️  Features

- *(download)* Introduce ability to download arbitrary files - ([7f7339a](https://github.com/QaidVoid/soar/commit/7f7339ab6d3d8a5aba7f8ba44997589ffd50fc94))
- *(run)* Run remote binary without metadata - ([695e0da](https://github.com/QaidVoid/soar/commit/695e0dac7e696f759722f2e3d173365446ab6a32))

### 🐛 Bug Fixes

- *(inspect)* Show error if log can't be fetched, and warn if log too large - ([82785fb](https://github.com/QaidVoid/soar/commit/82785fb5206c9491143544e76caa44e31c7c9122))
- *(run)* Fix run command - ([c2409fe](https://github.com/QaidVoid/soar/commit/c2409fe5136bd65079e45b1e0b5c47c921b44f94))

### 🚜 Refactor

- *(output)* Update command outputs - ([0967773](https://github.com/QaidVoid/soar/commit/09677738ff6ad1b6d7a10359dd2a4650e1b474a2))


## [0.1.0] - 2024-10-10

### ⛰️  Features

- *(cli)* Implement CLI commands structure - ([11f6214](https://github.com/QaidVoid/soar/commit/11f62145740ca7cdf8aa94b58aa48fa3b498e9f0))
- *(config)* Implement config loading - ([abbaaf6](https://github.com/QaidVoid/soar/commit/abbaaf66f2325641415487db1b4705e052300131))
- *(info)* Implement display installed package info - ([a79e9dd](https://github.com/QaidVoid/soar/commit/a79e9dd9709ebbcdd74349f02f0be2ae160d02e6))
- *(inspect)* Add command to inspect CI logs - ([50d6b60](https://github.com/QaidVoid/soar/commit/50d6b609abe37b421a353496be69637b1a022818))
- *(install)* Track and implement installed packages list - ([51e2f96](https://github.com/QaidVoid/soar/commit/51e2f968b4d9306154e61e2ebb44ea6df4483f1a))
- *(install)* Implement package install - ([aaf1c89](https://github.com/QaidVoid/soar/commit/aaf1c894f9c0caf5292afe9e7b4b1de2d5550d5e))
- *(list)* List available packages - ([17a50b7](https://github.com/QaidVoid/soar/commit/17a50b76cb921a026940ff8f8451a30e86dbb3cb))
- *(query)* Query detailed package info - ([0f6facd](https://github.com/QaidVoid/soar/commit/0f6facd18041485ce8ac6b56ad8b07f5e79afdf0))
- *(remove)* Implement packages removal - ([e676064](https://github.com/QaidVoid/soar/commit/e6760645621eea1119e48b073bb14f11c24b4b15))
- *(run)* Run packages without installing them - ([16e820a](https://github.com/QaidVoid/soar/commit/16e820a2145f7c2fa32d9deaf7621e813b2e1bb7))
- *(search)* Implement package search feature - ([313c2a5](https://github.com/QaidVoid/soar/commit/313c2a54c4149f948cb78b544299029f646a70e1))
- *(symlink)* Implement ownership check for binary symlinks - ([6575072](https://github.com/QaidVoid/soar/commit/65750728261d769d953ec9426d27ec53d5a8ed1a))
- *(update)* Implement update package - ([c58269b](https://github.com/QaidVoid/soar/commit/c58269b9a1a5668c68bb3ea93142c56f7a558276))
- *(use)* Add ability to switch package variants - ([de2264d](https://github.com/QaidVoid/soar/commit/de2264db461d85beab921179f1761abf49fe20cf))

### 🐛 Bug Fixes

- *(install)* Use case-sensitive package name - ([1abd650](https://github.com/QaidVoid/soar/commit/1abd6500073614e4adc245a1d97887bfa418df8e))
- *(parse)* Fix remote registry parser - ([b8175c5](https://github.com/QaidVoid/soar/commit/b8175c513c7bd4f4827ccf9a2df3defb5bdbbbd8))
- *(update)* Resolve update deadlock - ([e8c56bc](https://github.com/QaidVoid/soar/commit/e8c56bcf1ba913b832a4307f0329bf6564d61cff))

### 🚜 Refactor

- *(command)* Update commands and cleanup on sync - ([555737c](https://github.com/QaidVoid/soar/commit/555737c044f3cd0c4e5750808941f14621fe03d5))
- *(package)* Use binary checksum in install path - ([4a6e3c4](https://github.com/QaidVoid/soar/commit/4a6e3c406904df96a039860c83940ed7c66f6192))
- *(project)* Re-organize whole codebase - ([2705168](https://github.com/QaidVoid/soar/commit/270516888e8cff65b078f15bc91217ef5ee6b7d2))
- *(project)* Update data types and improve readability - ([ac4a93a](https://github.com/QaidVoid/soar/commit/ac4a93a01c7460331c98d844874020781cd5f074))
- *(project)* Reduce complexity - ([cfc5962](https://github.com/QaidVoid/soar/commit/cfc59628235d4600f4462357c3bbe48f4b3445e9))

### ⚙️ Miscellaneous Tasks

- *(README)* Add readme - ([9531d23](https://github.com/QaidVoid/soar/commit/9531d23049553fc9b04befe9ad939fd17a3ac02c))
- *(hooks)* Add cliff & git commit hooks - ([6757cf7](https://github.com/QaidVoid/soar/commit/6757cf75aa08e7b966503a142bbc4f1a44634902))


<!-- generated by git-cliff -->
