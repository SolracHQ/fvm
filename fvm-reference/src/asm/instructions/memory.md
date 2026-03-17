# Memory

## LOAD

Loads a value from the address held in `addr` into `dst`. Width is determined by `dst`. `addr`
must be an `rw` register since addresses are 32-bit.

```
LOAD rw, rw     # load 32-bit value at address in src
LOAD rh, rw     # load 16-bit value at address in src into low 16
LOAD rb, rw     # load byte at address in src into low 8
```

Encoding: `op dst_enc addr_enc` (3 bytes).

## STORE

Stores a value from `src` to the address held in `addr`. Width is determined by `src`. `addr`
must be an `rw` register since addresses are 32-bit.

```
STORE rw, rw    # store 32-bit value to address in addr
STORE rw, rh    # store low 16 of src to address in addr
STORE rw, rb    # store low 8 of src to address in addr
```

Encoding: `op addr_enc src_enc` (3 bytes).

## MMAP

Maps a range of physical pages into the context held in `mr`. Privileged.

```
MMAP rw, rw, rw     # virt_page, phys_page, page_count (all rw)
MMAP rw, rw, imm32  # page_count as immediate
```

- All operands are page numbers or page counts, not byte addresses. Multiply by 4096 to get
  the corresponding byte address.
- `virt_page` and `phys_page` must be `rw` registers.
- `page_count` may be an `rw` register or an `imm32`.
- Each covered page is mapped independently. Overlapping an existing mapping replaces it.
- The target context is `mr`, not `cr`. Set `mr` before calling `MMAP` when mapping into a
  context other than the current one.

## MUNMAP

Unmaps a range of pages from the context held in `mr`. Privileged.

```
MUNMAP rw, rw       # virt_page, page_count (both rw)
MUNMAP rw, imm32    # page_count as immediate
```

- `virt_page` must be an `rw` register.
- `page_count` may be an `rw` register or an `imm32`.
- Unmapping a page that is not mapped raises interrupt 1.
- The target context is `mr`, not `cr`.

## MPROTECT

Sets the permission bits on a range of pages in the context held in `mr`. Privileged.

```
MPROTECT rw, rw, rb     # virt_page, page_count, permission bits
MPROTECT rw, imm32, rb  # page_count as immediate
```

- The target context is `mr`, not `cr`.

Permission bits: bit 0 = Read, bit 1 = Write, bit 2 = Execute.
