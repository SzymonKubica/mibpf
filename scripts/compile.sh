clang -O2 -emit-llvm -c $1 -o - | llc -march=bpf -filetype=obj -o $2
