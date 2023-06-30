# png_decode

Project to get to know how PNG works, also implementing the deflate method to decompress the image. Documentation I used is on src/png.rs
Later I wanted to try JPEG, but was too long and had too edge cases to learn something about it apart from the hundred of pages the documentation has.
The program goes to png files in the test/image folder, and prints the png contents based on current width of terminal.


# Dependencies
`cargo add crossterm`

* crossterm: So I can access terminal from the rust program.
