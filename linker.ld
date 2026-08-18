ENTRY(kmain)

PHDRS
{
    text   PT_LOAD FLAGS(5); /* R + X */
    rodata PT_LOAD FLAGS(4); /* R */
    data   PT_LOAD FLAGS(6); /* R + W */
}

SECTIONS
{
    /*
     * Higher-half kernel virtual address.
     */
    . = 0xffffffff80000000;


    /*
     * ========================================================
     * TEXT
     * ========================================================
     */

    .text : ALIGN(4K)
    {
        /*
         * Kernel entry point must come first.
         */
        KEEP(*(.text.entry))

        /*
         * Rest of kernel code.
         */
        *(.text .text.*)
    } :text


    /*
     * ========================================================
     * RODATA
     * ========================================================
     */

    . = ALIGN(4K);

    .rodata : ALIGN(4K)
    {
        *(.rodata .rodata.*)


        /*
         * ----------------------------------------------------
         * Limine requests
         * ----------------------------------------------------
         *
         * These MUST remain in the final kernel image.
         */

        KEEP(*(.limine_req_start))
        KEEP(*(.limine_reqs))
        KEEP(*(.limine_req_end))

    } :rodata


    /*
     * ========================================================
     * KERNEL TESTS
     * ========================================================
     *
     * The #[test] procedural macro places a Test structure
     * into the .kernel_tests section.
     *
     * We expose the beginning and end of the section so
     * test.rs can iterate over every registered test.
     */

    .kernel_tests : ALIGN(8)
    {
        __kernel_tests_start = .;

        KEEP(*(.kernel_tests))

        __kernel_tests_end = .;

    } :rodata


    /*
     * ========================================================
     * DATA
     * ========================================================
     */

    . = ALIGN(4K);

    .data : ALIGN(4K)
    {
        *(.data .data.*)

    } :data


    /*
     * ========================================================
     * BSS
     * ========================================================
     */

    .bss : ALIGN(4K)
    {
        *(COMMON)

        *(.bss .bss.*)

    } :data


    /*
     * ========================================================
     * DISCARD
     * ========================================================
     *
     * We don't need unwind information in the kernel.
     */

    /DISCARD/ :
    {
        *(.eh_frame)
        *(.eh_frame_hdr)
    }
}
