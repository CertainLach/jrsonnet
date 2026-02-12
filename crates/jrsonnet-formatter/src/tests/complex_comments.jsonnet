{
		  comments: {
			_: '',
			//     Plain comment
			a: '',

			#    Plain comment with empty line before
			b: '',
			/*Single-line multiline comment

			*/
			c: '',

			/**Single-line multiline doc comment

			*/
			c: '',

			/**Multiline doc
			Comment
			*/
			c: '',

			/*

	Multi-line

	comment
			*/
			d: '',

			e: '', // Inline comment

			k: '',

			// Text after everything
		  },
		  comments2: {
			k: '',
			// Text after everything, but no newline above
		  },
          spacing: {
            a: '',

            b: '',
          },
          noSpacing: {
            a: '',
            b: '',
          },

			 smallObjectWithEnding: {/*Ending comment*/},
			 smallObjectWithFieldAndEnding: {a: 11/*Ending comment*/},
			 smallObjectWithFieldAndEnding2: {/*Start*/a: 11/*Ending comment*/},
        }
