
function draw_k_lines(dataset) {
  // same width as search bar
  var w = $("#search_bar").width(), h = 300;

  var svg;
  if (d3.select("svg#k_lines").empty()) {
    svg = d3.select("body")
      .append("svg")
      .attr("id", "k_lines")
      .attr("width", w)
      .attr("height", h);
  } else {
    svg = d3.select("svg#k_lines");
  }
  var price_max = d3.max(dataset, function(d) {
      return d.high;
  });
  var price_min = d3.min(dataset, function(d) {
      return d.low;
  })

  var yscale = d3.scaleLinear([price_min, price_max], [0, h]);

  svg.selectAll("rect").data(dataset).enter().append("rect")
      .attr("x", function(d, i) {
          return i * (w/dataset.length);
      })
      .attr('y', function(d, i) {
          return h - yscale(d3.max([d.open, d.close]));
      })
      .attr('width', function(d, i) {
          return w / dataset.length - 4;
      })
      .attr("height", function(d) {
          return Math.abs(yscale(d.open) - yscale(d.close));
      })
      .attr("fill", function(d) {
          if (d.open < d.close) return "red";
          return "green";
      });

  var data_cnt = dataset.length;
  var barPadding = 4;
  svg.selectAll("line")
      .data(dataset)
      .enter()
      .append("line")
      .attr("x1", function(d, i) {
          return i * (w / data_cnt) + (w / data_cnt - barPadding) / 2;
      })
      .attr("x2", function(d, i) {
          return i * (w / data_cnt) + (w / data_cnt - barPadding) / 2;
      })
      .attr("y1", function(d) {
          return h - yscale(d.high);
      })
      .attr("y2", function(d) {
          return h - yscale(d.low);
      })
      .attr("stroke", function(d) {
          if (d.open < d.close) return "red";
          return "green";
      });
}

function clear_k_lines() {
  d3.select("svg#k_lines").remove();
}

$(document).ready(function() {
  $("#input_stock_code").autocomplete({
    source: function(req, callback) {
      $.ajax({
        url: "api/v1/keyword-stocks?keyword=" + encodeURIComponent(req.term),
        method: "GET",
        dataType: "json",
        success: function(resp) {
          callback($.map(resp.content, function(item){
            return {
              value: item.code,
              label: item.code + " " + item.display_name
            };
          }));
        },
        error: function(err) {
          console.log("ajax error on search stock", err);
          callback([]);
        }
      })
    }
  });
  $("#input_start_dt").datepicker({
    dateFormat: "yy-mm-dd",
    minDate: "2010-01-01",
    maxDate: 0
  });
  $("#input_end_dt").datepicker({
    dateFormat: "yy-mm-dd",
    minDate: "2010-01-01",
    maxDate: 0
  });
  $("#stock_submission").click(function() {
    var input_stock_code = $("#input_stock_code").val();
    var input_start_dt = $("#input_start_dt").val();
    var input_end_dt = $("#input_end_dt").val();
    var input_tick = $("#input_tick option:selected").text();    
    $.ajax({
      url: "api/v1/stock-prices/" + encodeURIComponent(input_stock_code)
        + "/ticks/" + encodeURIComponent(input_tick) + "?start_dt="
        + encodeURIComponent(input_start_dt) + "&end_dt="
        + encodeURIComponent(input_end_dt),
      method: "GET",
      dataType: "json",
      success: function(resp) {
        var dataset = $.map(resp.data, function(item) {
          return {
            date: item.ts,
            open: parseFloat(item.open),
            close: parseFloat(item.close),
            high: parseFloat(item.high),
            low: parseFloat(item.low)
          };
        });
        draw_k_lines(dataset);
      },
      error: function(err) {
        console.log("ajax error on query prices", err);
        clear_k_lines();
      }
    })
  });

});

