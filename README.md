# Sandbox



## Usage

### build

build/compile code with compile time limitation

`sandbox build`

#### Arguments

| Long | Short | Description | Required |
|------|-------|-------------|----------|
|language|l|language of source code|Y|
|input|i|source code input file|Y|
|output|o|compile output path|Y|
|time||compile time limit in second (default: 15)||

### run

execute binary or script with safety

`sandbox run`

#### Arguments

| Long | Short | Description | Required |
|------|-------|-------------|----------|
|language|l|language of executable is written|Y|
|file|f|path of file to execute|Y|
|time||runtime limit in second|Y|
|memory||memory limit in bytes|Y|
|input|i|Test case file path||
|output|o|File path to pipe stdout||
|answer|a|answer file path to compare with output||
|env|e|environment variables||
|workdir||working directory||
|rootdir||root directory||