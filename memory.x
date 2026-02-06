/* Memory layout for STM32F103C8T6 */
MEMORY
{
  /* Flash: 64KB */
  FLASH : ORIGIN = 0x08000000, LENGTH = 64K
  
  /* RAM: 20KB */
  RAM : ORIGIN = 0x20000000, LENGTH = 20K
}

/* defmt logging support */
SECTIONS
{
  /* Place defmt data at the end of FLASH after all code */
  .defmt (NOLOAD) : ALIGN(4)
  {
    *(.defmt .defmt.*);
  } > RAM
}
