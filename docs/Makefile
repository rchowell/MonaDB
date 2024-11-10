ASCIIDOCTOR=asciidoctor
SRC=$(wildcard *.adoc)
BUILD_DIR=build
HTML=$(SRC:%.adoc=$(BUILD_DIR)/%.html)

.PHONY: all build clean

all: build

build: $(HTML)

$(BUILD_DIR)/%.html: %.adoc
	@mkdir -p $(BUILD_DIR)
	$(ASCIIDOCTOR) -a stylesheet=txt.css -o $@ $<

clean:
	rm -rf $(BUILD_DIR)
