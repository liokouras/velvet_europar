CC = ${OPENCILK_HOME}/bin/clang

CFLAGS = -L${OPENCILK_HOME}/lib -L${OPENCILK_HOME}/lib64 -fopencilk -O3 -fopencilk
LDFLAGS = -lm -lpthread -ldl

RUST_LIB_DIR = ../../sort_rs/target/release
RUST_LIB = sort_rs

TARGET = ../../zout/sort_cilk
SRC = sort.c

all: $(TARGET)

$(TARGET): $(SRC)
	$(CC) $(CFLAGS) $(SRC) \
		-L$(RUST_LIB_DIR) -l$(RUST_LIB) \
		$(LDFLAGS) \
		-o $(TARGET)

clean:
	rm -f $(TARGET)
