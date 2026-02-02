#!/bin/bash

echo "GATHERING OSIRIS SOURCE CODE..."

# Define the files to gather
FILES=$(find src -type f -name "*.rs" | tr ' ' '\n' )

# Output destination
OUT="osiris_dump.txt"
echo "Project OSIRIS Source Dump - $(date)" > $OUT
echo "-----------------------------------" >> $OUT

for f in ${FILES}; do
    if [ -f "$f" ]; then
        echo "Processing $f..."
        echo -e "\n**$f**" >> $OUT
        echo "\`\`\`rust" >> $OUT # Note: using rust for all for highlighting
        cat "$f" >> $OUT
        echo -e "\n\`\`\`" >> $OUT
    else
        echo "Warning: $f not found!" >> $OUT
    fi
done

# Try to copy to clipboard (macOS)
if command -v pbcopy > /dev/null; then
    cat $OUT | pbcopy
    echo "SUCCESS: All code copied to clipboard. Paste it into the chat."
else
    echo "SUCCESS: All code gathered in $OUT. Copy the contents of that file."
fi